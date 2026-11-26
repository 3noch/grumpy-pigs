//! Level links: a compact, versioned, forward-compatible encoding of a
//! [`Level`] that fits in a URL fragment (`…/grumpy-pigs/#L1.<data>`).
//!
//! # Format (version 1)
//!
//! The text form is `L1.` followed by the bytes below in unpadded base64url.
//!
//! ```text
//! bytes   := record* crc16
//! record  := tag:varint  len:varint  payload[len]
//! ```
//!
//! **Even tags are required, odd tags are optional.** A decoder skips
//! optional records it doesn't know, and refuses levels containing required
//! records it doesn't know ("needs a newer version of the game"). New
//! cosmetic data (say, a decoration layer) gets an odd tag; new gameplay
//! (say, portals) gets an even one.
//!
//! | tag | meaning  | payload |
//! |-----|----------|---------|
//! | 1   | name     | UTF-8 |
//! | 3   | author   | UTF-8 |
//! | 5   | theme    | varint theme id (unknown ids fall back to Meadow) |
//! | 7   | based on | UTF-8: author of the level this one remixes |
//! | 2   | pigs     | varint pig ids, in launch order |
//! | 4   | pieces   | piece* |
//! | 6   | launcher | x:zigzag y:zigzag (cm; the point under the sling's base), then future fields. Omitted = (6 m, 0) |
//!
//! ```text
//! piece   := len:varint  kind:varint  x:zigzag  y:zigzag  angle:varint  size…  (future fields…)
//! size    := w:varint h:varint     for box pieces
//!          | r:varint              for round pieces (wolves, boulders)
//! ```
//!
//! Lengths are in centimeters; `angle` is in 1/65536ths of a full turn, so
//! any angle is representable. Each piece carries its own length so future
//! versions can append fields that older decoders ignore. Piece kinds follow
//! the same parity rule as tags: even kinds are required, odd kinds are
//! optional (skipped when unknown).
//!
//! The trailing CRC-16 (CCITT-FALSE) over all preceding bytes catches links
//! that were truncated or mangled in transit.

use std::f32::consts::TAU;

use crate::sim::{Launcher, Level, Material, Piece, PieceKind, PigKind, Size, Theme, WolfKind};

pub const PREFIX: &str = "L1.";

const TAG_NAME: u64 = 1;
const TAG_AUTHOR: u64 = 3;
const TAG_THEME: u64 = 5;
const TAG_BASED_ON: u64 = 7;
const TAG_PIGS: u64 = 2;
const TAG_PIECES: u64 = 4;
const TAG_LAUNCHER: u64 = 6;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DecodeError {
    /// Not a level link at all.
    NotALevel,
    /// Truncated or corrupted.
    Corrupt,
    /// Made by a newer version of the game, with parts this version lacks.
    NeedsNewerGame,
}

impl DecodeError {
    pub fn message(self) -> &'static str {
        match self {
            DecodeError::NotALevel => "That isn't a Grumpy Pigs level link.",
            DecodeError::Corrupt => "This level link looks broken or cut off.",
            DecodeError::NeedsNewerGame => "This level needs a newer version of the game.",
        }
    }
}

// --- Ids. Never renumber these: links in the wild depend on them. ---

fn piece_kind_id(kind: PieceKind) -> u64 {
    match kind {
        PieceKind::Terrain => 0,
        PieceKind::Block(Material::Straw) => 2,
        PieceKind::Block(Material::Wood) => 4,
        PieceKind::Block(Material::Brick) => 6,
        PieceKind::Block(Material::Ice) => 8,
        PieceKind::Block(Material::Tnt) => 10,
        PieceKind::Block(Material::Stone) => 12,
        PieceKind::Boulder => 14,
        PieceKind::Wolf(WolfKind::Pup) => 16,
        PieceKind::Wolf(WolfKind::Wolf) => 18,
        PieceKind::Wolf(WolfKind::Helmet) => 20,
        PieceKind::Wolf(WolfKind::Boss) => 22,
        PieceKind::Fairy => 24,
    }
}

fn piece_kind_from_id(id: u64) -> Option<PieceKind> {
    Some(match id {
        0 => PieceKind::Terrain,
        2 => PieceKind::Block(Material::Straw),
        4 => PieceKind::Block(Material::Wood),
        6 => PieceKind::Block(Material::Brick),
        8 => PieceKind::Block(Material::Ice),
        10 => PieceKind::Block(Material::Tnt),
        12 => PieceKind::Block(Material::Stone),
        14 => PieceKind::Boulder,
        16 => PieceKind::Wolf(WolfKind::Pup),
        18 => PieceKind::Wolf(WolfKind::Wolf),
        20 => PieceKind::Wolf(WolfKind::Helmet),
        22 => PieceKind::Wolf(WolfKind::Boss),
        24 => PieceKind::Fairy,
        _ => return None,
    })
}

/// Whether a kind is stored with a radius rather than width and height.
fn is_round(kind: PieceKind) -> bool {
    matches!(
        kind,
        PieceKind::Boulder | PieceKind::Wolf(_) | PieceKind::Fairy
    )
}

fn pig_id(kind: PigKind) -> u64 {
    match kind {
        PigKind::Grunt => 0,
        PigKind::Dasher => 1,
        PigKind::Boomer => 2,
        PigKind::Hog => 3,
        PigKind::Litter => 4,
        PigKind::Thumper => 5,
        PigKind::Puffer => 6,
        PigKind::Piglet => 7,
    }
}

fn pig_from_id(id: u64) -> Option<PigKind> {
    Some(match id {
        0 => PigKind::Grunt,
        1 => PigKind::Dasher,
        2 => PigKind::Boomer,
        3 => PigKind::Hog,
        4 => PigKind::Litter,
        5 => PigKind::Thumper,
        6 => PigKind::Puffer,
        7 => PigKind::Piglet,
        _ => return None,
    })
}

fn theme_id(theme: Theme) -> u64 {
    match theme {
        Theme::Meadow => 0,
    }
}

fn theme_from_id(_id: u64) -> Theme {
    // Unknown themes are cosmetic: fall back to the default.
    Theme::Meadow
}

// --- Encoding ---

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn put_zigzag(out: &mut Vec<u8>, v: i64) {
    #[allow(clippy::cast_sign_loss)]
    put_varint(out, ((v << 1) ^ (v >> 63)) as u64);
}

fn put_record(out: &mut Vec<u8>, tag: u64, payload: &[u8]) {
    put_varint(out, tag);
    put_varint(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

/// Meters → whole centimeters.
#[allow(clippy::cast_possible_truncation)]
fn cm(v: f32) -> i64 {
    (f64::from(v) * 100.0).round() as i64
}

/// Non-negative meters → whole centimeters.
fn cm_len(v: f32) -> u64 {
    u64::try_from(cm(v).max(0)).unwrap_or(0)
}

/// Radians → 1/65536ths of a turn.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn turn_units(angle: f32) -> u64 {
    let turns = (f64::from(angle) / f64::from(TAU)).rem_euclid(1.0);
    ((turns * 65536.0).round() as u64) % 65536
}

fn encode_piece(p: &Piece) -> Vec<u8> {
    let mut b = Vec::with_capacity(12);
    put_varint(&mut b, piece_kind_id(p.kind));
    put_zigzag(&mut b, cm(p.x));
    put_zigzag(&mut b, cm(p.y));
    put_varint(&mut b, turn_units(p.angle));
    if is_round(p.kind) {
        put_varint(&mut b, cm_len(p.size.radius()));
    } else {
        let (w, h) = match p.size {
            Size::Box { w, h } => (w, h),
            Size::Ball { r } => (r * 2.0, r * 2.0),
        };
        put_varint(&mut b, cm_len(w));
        put_varint(&mut b, cm_len(h));
    }
    b
}

/// The raw bytes of a level (without prefix or base64).
pub fn encode_bytes(level: &Level) -> Vec<u8> {
    let mut out = Vec::new();
    if !level.name.is_empty() {
        put_record(&mut out, TAG_NAME, level.name.as_bytes());
    }
    if !level.author.is_empty() {
        put_record(&mut out, TAG_AUTHOR, level.author.as_bytes());
    }
    if !level.based_on.is_empty() {
        put_record(&mut out, TAG_BASED_ON, level.based_on.as_bytes());
    }
    if level.theme != Theme::default() {
        let mut b = Vec::new();
        put_varint(&mut b, theme_id(level.theme));
        put_record(&mut out, TAG_THEME, &b);
    }
    if level.launcher != Launcher::default() {
        let mut b = Vec::new();
        put_zigzag(&mut b, cm(level.launcher.x));
        put_zigzag(&mut b, cm(level.launcher.y));
        put_record(&mut out, TAG_LAUNCHER, &b);
    }
    let mut pigs = Vec::new();
    for &k in &level.pigs {
        put_varint(&mut pigs, pig_id(k));
    }
    put_record(&mut out, TAG_PIGS, &pigs);
    let mut pieces = Vec::new();
    for p in &level.pieces {
        let b = encode_piece(p);
        put_varint(&mut pieces, b.len() as u64);
        pieces.extend_from_slice(&b);
    }
    put_record(&mut out, TAG_PIECES, &pieces);
    let crc = crc16(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

/// The shareable text form, e.g. `L1.AbC…`.
pub fn encode(level: &Level) -> String {
    let mut s = String::from(PREFIX);
    s.push_str(&base64url_encode(&encode_bytes(level)));
    s
}

// --- Decoding ---

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }
    fn varint(&mut self) -> Result<u64, DecodeError> {
        let mut v: u64 = 0;
        for shift in (0..64).step_by(7) {
            let &byte = self.buf.get(self.pos).ok_or(DecodeError::Corrupt)?;
            self.pos += 1;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError::Corrupt)
    }
    fn zigzag(&mut self) -> Result<i64, DecodeError> {
        let v = self.varint()?;
        #[allow(clippy::cast_possible_wrap)]
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }
    fn bytes(&mut self, len: u64) -> Result<&'a [u8], DecodeError> {
        let len = usize::try_from(len).map_err(|_| DecodeError::Corrupt)?;
        let end = self.pos.checked_add(len).ok_or(DecodeError::Corrupt)?;
        let s = self.buf.get(self.pos..end).ok_or(DecodeError::Corrupt)?;
        self.pos = end;
        Ok(s)
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn meters(cm: i64) -> f32 {
    (cm as f64 / 100.0) as f32
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn meters_len(cm: u64) -> f32 {
    (cm as f64 / 100.0) as f32
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn radians(units: u64) -> f32 {
    ((units % 65536) as f64 / 65536.0 * f64::from(TAU)) as f32
}

/// Decodes one piece; `Ok(None)` means an unknown optional kind to skip.
fn decode_piece(b: &[u8]) -> Result<Option<Piece>, DecodeError> {
    let mut r = Reader::new(b);
    let id = r.varint()?;
    let Some(kind) = piece_kind_from_id(id) else {
        return if id % 2 == 1 {
            Ok(None)
        } else {
            Err(DecodeError::NeedsNewerGame)
        };
    };
    let x = meters(r.zigzag()?);
    let y = meters(r.zigzag()?);
    let angle = radians(r.varint()?);
    let size = if is_round(kind) {
        Size::Ball {
            r: meters_len(r.varint()?),
        }
    } else {
        Size::Box {
            w: meters_len(r.varint()?),
            h: meters_len(r.varint()?),
        }
    };
    // Any remaining bytes are fields from a newer format version.
    Ok(Some(Piece {
        kind,
        x,
        y,
        angle,
        size,
    }))
}

pub fn decode_bytes(bytes: &[u8]) -> Result<Level, DecodeError> {
    let (body, crc) = bytes.split_last_chunk::<2>().ok_or(DecodeError::Corrupt)?;
    if crc16(body) != u16::from_be_bytes(*crc) {
        return Err(DecodeError::Corrupt);
    }
    let mut level = Level::default();
    let mut r = Reader::new(body);
    while !r.done() {
        let tag = r.varint()?;
        let len = r.varint()?;
        let payload = r.bytes(len)?;
        match tag {
            TAG_NAME => level.name = String::from_utf8_lossy(payload).into_owned(),
            TAG_AUTHOR => level.author = String::from_utf8_lossy(payload).into_owned(),
            TAG_THEME => level.theme = theme_from_id(Reader::new(payload).varint()?),
            TAG_BASED_ON => level.based_on = String::from_utf8_lossy(payload).into_owned(),
            TAG_PIGS => {
                let mut p = Reader::new(payload);
                while !p.done() {
                    let pig = pig_from_id(p.varint()?).ok_or(DecodeError::NeedsNewerGame)?;
                    level.pigs.push(pig);
                }
            }
            TAG_LAUNCHER => {
                // Any bytes after x and y are fields from a newer version.
                let mut p = Reader::new(payload);
                level.launcher = Launcher {
                    x: meters(p.zigzag()?),
                    y: meters(p.zigzag()?),
                };
            }
            TAG_PIECES => {
                let mut p = Reader::new(payload);
                while !p.done() {
                    let len = p.varint()?;
                    if let Some(piece) = decode_piece(p.bytes(len)?)? {
                        level.pieces.push(piece);
                    }
                }
            }
            t if t % 2 == 1 => {} // unknown optional record
            _ => return Err(DecodeError::NeedsNewerGame),
        }
    }
    Ok(level)
}

/// Decodes a level from a link, a bare `L1.…` code, or anything containing one
/// (a full URL, a message with the URL pasted in, …).
pub fn decode(text: &str) -> Result<Level, DecodeError> {
    let code = find_code(text)?;
    let bytes = base64url_decode(code).ok_or(DecodeError::Corrupt)?;
    decode_bytes(&bytes)
}

/// Finds the `L<version>.<data>` code inside arbitrary text and returns the
/// data part, if it's a version this game understands.
fn find_code(text: &str) -> Result<&str, DecodeError> {
    let text = text.trim();
    let start = match text.find("#L") {
        Some(i) => i + 1,
        None if text.starts_with('L') => 0,
        None => return Err(DecodeError::NotALevel),
    };
    let rest = &text[start + 1..];
    let dot = rest.find('.').ok_or(DecodeError::NotALevel)?;
    let version = &rest[..dot];
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit()) {
        return Err(DecodeError::NotALevel);
    }
    if format!("L{version}.") != PREFIX {
        return Err(DecodeError::NeedsNewerGame);
    }
    let data = &rest[dot + 1..];
    let end = data
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '=')))
        .unwrap_or(data.len());
    Ok(&data[..end])
}

// --- Helpers ---

/// CRC-16/CCITT-FALSE.
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// Accepts base64url or standard base64, with or without padding.
fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels;

    #[test]
    fn every_builtin_level_round_trips() {
        for lvl in levels::all() {
            let code = encode(&lvl);
            let back = decode(&code).expect("decodes");
            // Quantized to centimeters, so compare by re-encoding.
            assert_eq!(encode(&back), code, "{}", lvl.name);
            assert_eq!(back.name, lvl.name);
            assert_eq!(back.pieces.len(), lvl.pieces.len());
            assert_eq!(back.pigs, lvl.pigs);
            for (a, b) in lvl.pieces.iter().zip(&back.pieces) {
                assert_eq!(a.kind, b.kind);
                assert!((a.x - b.x).abs() < 0.006 && (a.y - b.y).abs() < 0.006);
                let d = (a.angle.rem_euclid(TAU) - b.angle).abs();
                assert!(d < 0.001 || TAU - d < 0.001, "{} vs {}", a.angle, b.angle);
            }
            eprintln!("{:>14}: {} chars", lvl.name, code.len());
        }
    }

    #[test]
    fn links_are_found_inside_urls_and_messages() {
        let lvl = &levels::all()[0];
        let code = encode(lvl);
        for text in [
            format!("https://3noch.github.io/grumpy-pigs/#{code}"),
            format!("Try this! https://example.com/#{code} so hard"),
            format!("  {code}\n"),
        ] {
            assert_eq!(decode(&text).unwrap().name, lvl.name, "{text}");
        }
        assert_eq!(decode("hello"), Err(DecodeError::NotALevel));
        assert_eq!(
            decode("https://x/#L9.AAAA"),
            Err(DecodeError::NeedsNewerGame)
        );
        assert_eq!(decode("Look! #Lunch.time"), Err(DecodeError::NotALevel));
    }

    #[test]
    fn launcher_position_round_trips() {
        let mut lvl = levels::all()[3].clone();
        let default_code = encode(&lvl);
        lvl.launcher = Launcher { x: 9.5, y: 4.25 };
        let code = encode(&lvl);
        assert_ne!(code, default_code);
        assert_eq!(decode(&code).unwrap().launcher, lvl.launcher);
        // Negative coordinates work too ("no limits").
        lvl.launcher = Launcher { x: -3.0, y: -0.5 };
        assert_eq!(decode(&encode(&lvl)).unwrap().launcher, lvl.launcher);
    }

    #[test]
    fn full_range_angles_survive() {
        let mut lvl = Level::default();
        for i in 0..36 {
            lvl.pieces.push(Piece {
                kind: PieceKind::Terrain,
                x: 1.0,
                y: 2.0,
                angle: (i as f32).to_radians() * 10.0 + 0.123,
                size: Size::Box { w: 3.0, h: 0.5 },
            });
        }
        let back = decode(&encode(&lvl)).unwrap();
        for (a, b) in lvl.pieces.iter().zip(&back.pieces) {
            let d = (a.angle.rem_euclid(TAU) - b.angle).abs();
            assert!(
                d < 0.0002 || (TAU - d) < 0.0002,
                "{} vs {}",
                a.angle,
                b.angle
            );
        }
    }

    #[test]
    fn truncated_or_mangled_links_are_rejected() {
        let code = encode(&levels::all()[4]);
        for cut in [code.len() - 1, code.len() - 5, code.len() / 2, 4] {
            assert!(decode(&code[..cut]).is_err(), "cut at {cut}");
        }
        let mut chars: Vec<char> = code.chars().collect();
        let i = chars.len() / 2;
        chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
        let mangled: String = chars.into_iter().collect();
        assert_eq!(decode(&mangled), Err(DecodeError::Corrupt));
    }

    /// Re-signs bytes so tests can craft records.
    fn with_crc(mut body: Vec<u8>) -> Vec<u8> {
        let crc = crc16(&body);
        body.extend_from_slice(&crc.to_be_bytes());
        body
    }

    #[test]
    fn unknown_optional_data_is_skipped_and_required_data_refused() {
        let lvl = &levels::all()[0];
        let mut body = encode_bytes(lvl);
        body.truncate(body.len() - 2);

        // Unknown optional record (odd tag 99): ignored.
        let mut opt = body.clone();
        put_record(&mut opt, 99, b"sparkles");
        assert_eq!(
            decode_bytes(&with_crc(opt)).unwrap().pieces.len(),
            lvl.pieces.len()
        );

        // Unknown required record (even tag 98): refused.
        let mut req = body.clone();
        put_record(&mut req, 98, b"portals");
        assert_eq!(
            decode_bytes(&with_crc(req)),
            Err(DecodeError::NeedsNewerGame)
        );

        // Unknown optional piece kind (odd), and a known piece with extra
        // trailing fields from the future: both fine.
        let mut pieces = Vec::new();
        let mut deco = Vec::new();
        put_varint(&mut deco, 31);
        deco.extend_from_slice(&[1, 2, 3]);
        put_varint(&mut pieces, deco.len() as u64);
        pieces.extend_from_slice(&deco);
        let mut future = encode_piece(&lvl.pieces[0]);
        future.extend_from_slice(&[7, 7, 7]);
        put_varint(&mut pieces, future.len() as u64);
        pieces.extend_from_slice(&future);
        let mut b = Vec::new();
        put_record(&mut b, TAG_PIECES, &pieces);
        let back = decode_bytes(&with_crc(b)).unwrap();
        assert_eq!(back.pieces.len(), 1);
        assert_eq!(back.pieces[0].kind, lvl.pieces[0].kind);

        // Unknown required piece kind (even): refused.
        let mut pieces = Vec::new();
        let mut portal = Vec::new();
        put_varint(&mut portal, 40);
        put_varint(&mut pieces, portal.len() as u64);
        pieces.extend_from_slice(&portal);
        let mut b = Vec::new();
        put_record(&mut b, TAG_PIECES, &pieces);
        assert_eq!(decode_bytes(&with_crc(b)), Err(DecodeError::NeedsNewerGame));
    }

    #[test]
    fn random_garbage_never_panics() {
        let mut seed: u64 = 0x1234_5678;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..20_000 {
            let len = usize::try_from(next() % 64).unwrap();
            let bytes: Vec<u8> = (0..len).map(|_| next().to_le_bytes()[0]).collect();
            let _ = decode_bytes(&bytes);
            let _ = decode_bytes(&with_crc(bytes.clone()));
            let _ = decode(&format!("L1.{}", base64url_encode(&bytes)));
        }
    }

    /// `cargo test print_codes -- --ignored --nocapture` lists each built-in
    /// level's link code (handy for testing links by hand).
    #[test]
    #[ignore = "dev aid, prints codes"]
    fn print_codes() {
        for lvl in levels::all() {
            println!("{}", encode(&lvl));
        }
    }

    #[test]
    fn empty_and_unicode_levels_work() {
        let lvl = Level {
            name: "Ünïcødé 🐷 level".into(),
            author: "Zoë".into(),
            based_on: "Bjørn".into(),
            ..Level::default()
        };
        let back = decode(&encode(&lvl)).unwrap();
        assert_eq!(back, lvl);
    }
}
