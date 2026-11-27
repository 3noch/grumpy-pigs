//! Character art: shaded, outlined, animated pigs and wolves.
//!
//! Everything is drawn in a local frame where the character's radius is 1.0,
//! so the same code works for every size, and squash/stretch applies to all
//! features at once.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use crate::gfx::{P, Rig, alpha, bez, blink, hash, hsv, mix, rgb, rgba};
use crate::render::View;
use crate::sim::{PigKind, Portal, PortalEnd, V2, WolfKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    /// Pig: grumbling. Wolf: smug.
    Idle,
    /// Pig on the sling: gritted teeth.
    Aim,
    /// Pig in flight: battle cry.
    Yell,
    /// Pig after a crash.
    Dizzy,
    /// Pigs after a win.
    Cheer,
    /// Wolf sees a pig coming, or is badly hurt.
    Worried,
    /// Wolf just took a hit.
    Hurt,
    /// Wolf mocking a missed shot.
    Laugh,
}

#[derive(Clone, Copy)]
pub struct Anim {
    pub t: f32,
    pub seed: u32,
    pub mood: Mood,
    /// World-space direction the character looks toward.
    pub look: V2,
    /// Squash/stretch along `stretch_dir` (0 = none, + = elongate).
    pub stretch: f32,
    pub stretch_dir: V2,
    /// 0..1 red-hot glow (Boomer about to blow).
    pub glow: f32,
    /// Size multiplier on top of the kind's radius (Puffer inflation).
    pub scale: f32,
}

impl Anim {
    pub fn new(t: f32, seed: u32, mood: Mood, look: V2) -> Self {
        Anim {
            t,
            seed,
            mood,
            look,
            stretch: 0.0,
            stretch_dir: V2::Y,
            glow: 0.0,
            scale: 1.0,
        }
    }
}

pub(crate) fn rig_for(view: &View, pos: V2, angle: f32, r: f32, a: &Anim, bounce: f32) -> Rig {
    let rig = Rig::new(*view, pos, angle, r);
    if a.stretch.abs() > 0.005 {
        rig.stretched(a.stretch_dir, a.stretch)
    } else {
        // Idle breathing.
        let phase = hash(a.seed, 3) * TAU;
        rig.stretched(V2::Y, 0.03 * (a.t * 2.4 + phase).sin() + bounce)
    }
}

/// Pupil offset in local space toward the look direction.
pub(crate) fn gaze(rig: &Rig, look: V2, reach: f32) -> P {
    let d = rig.local_dir(look).normalize_or_zero() * reach;
    (d.x, d.y)
}

struct PigPalette {
    light: Color,
    base: Color,
    dark: Color,
    line: Color,
}

fn pig_palette(kind: PigKind) -> PigPalette {
    match kind {
        PigKind::Grunt => PigPalette {
            light: rgb(255, 214, 222),
            base: rgb(242, 140, 168),
            dark: rgb(214, 98, 132),
            line: rgb(110, 40, 62),
        },
        PigKind::Dasher => PigPalette {
            light: rgb(255, 236, 160),
            base: rgb(250, 182, 72),
            dark: rgb(222, 132, 44),
            line: rgb(112, 60, 22),
        },
        PigKind::Boomer => PigPalette {
            light: rgb(158, 142, 182),
            base: rgb(96, 82, 118),
            dark: rgb(66, 54, 86),
            line: rgb(30, 24, 40),
        },
        PigKind::Hog => PigPalette {
            light: rgb(214, 160, 118),
            base: rgb(160, 100, 66),
            dark: rgb(118, 70, 44),
            line: rgb(56, 30, 18),
        },
        PigKind::Litter | PigKind::Piglet => PigPalette {
            light: rgb(255, 240, 222),
            base: rgb(244, 206, 172),
            dark: rgb(214, 160, 122),
            line: rgb(102, 58, 36),
        },
        PigKind::Thumper => PigPalette {
            light: rgb(228, 196, 206),
            base: rgb(178, 128, 150),
            dark: rgb(138, 92, 114),
            line: rgb(58, 28, 44),
        },
        PigKind::Puffer => PigPalette {
            light: rgb(214, 252, 228),
            base: rgb(124, 208, 168),
            dark: rgb(80, 162, 122),
            line: rgb(26, 78, 58),
        },
    }
}

/// Ear polygon: curved-sided triangle from a base center, pointing at `ang`.
fn ear(base: P, ang: f32, len: f32, half_w: f32) -> Vec<P> {
    let (s, c) = ang.sin_cos();
    let (nx, ny) = (-s, c);
    let bl = (base.0 + nx * half_w, base.1 + ny * half_w);
    let br = (base.0 - nx * half_w, base.1 - ny * half_w);
    let tip = (base.0 + c * len, base.1 + s * len);
    let bulge = half_w * 0.9;
    let c1 = (
        base.0 + c * len * 0.55 + nx * bulge,
        base.1 + s * len * 0.55 + ny * bulge,
    );
    let c2 = (
        base.0 + c * len * 0.55 - nx * bulge,
        base.1 + s * len * 0.55 - ny * bulge,
    );
    let mut pts = bez(bl, c1, tip, 6);
    pts.extend(bez(tip, c2, br, 6).into_iter().skip(1));
    pts
}

fn shrink(pts: &[P], k: f32, toward: P) -> Vec<P> {
    pts.iter()
        .map(|&(x, y)| (toward.0 + (x - toward.0) * k, toward.1 + (y - toward.1) * k))
        .collect()
}

/// A furious pig.
#[allow(clippy::too_many_lines)]
pub fn pig(view: &View, kind: PigKind, pos: V2, angle: f32, a: &Anim) {
    let t = a.t;
    let cheer_hop = if a.mood == Mood::Cheer {
        0.08 * (t * 9.0).sin()
    } else {
        0.0
    };
    let rig = rig_for(view, pos, angle, kind.radius() * a.scale, a, cheer_hop);
    let mut pal = pig_palette(kind);
    if a.glow > 0.0 {
        let pulse = 0.5 + 0.5 * (t * 25.0).sin();
        let hot = rgb(255, 70, 40);
        pal.base = mix(pal.base, hot, a.glow * pulse * 0.8);
        pal.light = mix(pal.light, rgb(255, 200, 120), a.glow * pulse * 0.6);
    }
    let ow = 0.07;
    let look = gaze(&rig, a.look, 0.07);
    let back = if look.0 >= 0.0 { -1.0 } else { 1.0 };

    // --- Behind the head ---
    if kind == PigKind::Dasher {
        // Headband tails streaming behind, waving.
        let knot = (back * 0.85, 0.5);
        for (i, lift) in [0.18, -0.05].iter().enumerate() {
            let wave = (t * 14.0 + i as f32 * 1.7).sin() * 0.12;
            let end = (knot.0 + back * 0.6, knot.1 + lift + wave);
            let mid = (knot.0 + back * 0.3, knot.1 + lift * 0.5 - wave);
            let pts = bez(knot, mid, end, 8);
            rig.stroke(&pts, 0.17, pal.line);
            rig.stroke(&pts, 0.11, rgb(222, 44, 50));
        }
    }

    if kind == PigKind::Puffer {
        // Spines that bristle out as it inflates.
        let puffed = ((a.scale - 1.0) / 1.2).clamp(0.0, 1.0);
        let len = 0.12 + 0.22 * puffed;
        for i in 0..16 {
            let th = i as f32 / 16.0 * TAU + 0.1;
            let (c, s) = (th.cos(), th.sin());
            let (tc, ts) = ((th + 0.12).cos(), (th + 0.12).sin());
            let (bc, bs) = ((th - 0.12).cos(), (th - 0.12).sin());
            let tip = (c * (1.0 + len), s * (1.0 + len));
            rig.poly(
                &[(bc * 0.95, bs * 0.95), tip, (tc * 0.95, ts * 0.95)],
                pal.dark,
                Some((pal.line, 0.03)),
            );
        }
    }

    // Ears flap in flight, twitch when idle.
    let flap = match a.mood {
        Mood::Yell => (t * 22.0).sin() * 0.35,
        Mood::Cheer => (t * 12.0).sin() * 0.25,
        Mood::Dizzy => -0.3,
        _ => (t * 2.0 + hash(a.seed, 7) * 6.0).sin() * 0.08,
    };
    for sx in [-1.0f32, 1.0] {
        let ang = PI / 2.0 - sx * (0.65 + flap * sx.signum());
        let base = (sx * 0.45, 0.68);
        let pts = ear(base, ang, 0.55, 0.22);
        rig.poly(&pts, pal.base, Some((pal.line, ow)));
        let inner = shrink(
            &pts,
            0.55,
            (base.0 + ang.cos() * 0.15, base.1 + ang.sin() * 0.15),
        );
        rig.poly(&inner, pal.dark, None);
    }

    let pilgrim = crate::holiday::thanksgiving();
    if pilgrim {
        pilgrim_collar(&rig, pal.line);
    }

    // --- Head ---
    let bristles = kind == PigKind::Hog;
    let head = move |th: f32| {
        let mut r = 1.0;
        if bristles && th > 0.6 && th < PI - 0.6 {
            let tri = ((th * 11.0) % 1.0 - 0.5).abs() * 2.0;
            r += 0.13 * tri;
        }
        r
    };
    rig.radial((0.0, 0.0), head, ow, pal.line, pal.line, (0.0, 0.0));
    rig.radial((0.0, 0.0), head, 0.0, pal.light, pal.base, (-0.35, 0.4));
    // Rim shade along the bottom and a glossy highlight.
    rig.ellipse(0.0, -0.78, 0.62, 0.16, 0.0, alpha(pal.dark, 0.45));
    rig.ellipse(-0.45, 0.5, 0.2, 0.11, 0.7, rgba(255, 255, 255, 0.45));

    // Cheeks (the Puffer's swell up as it inflates)
    let cheek = if kind == PigKind::Puffer {
        0.17 * a.scale.min(1.6)
    } else {
        0.17
    };
    for sx in [-1.0, 1.0] {
        rig.ellipse(
            sx * 0.62,
            -0.18,
            cheek,
            cheek * 0.6,
            0.0,
            rgba(255, 80, 110, 0.3),
        );
    }
    if matches!(kind, PigKind::Litter | PigKind::Piglet) {
        // Brown patches
        for (x, y, rx, ry) in [
            (0.55, 0.45, 0.22, 0.17),
            (-0.62, 0.05, 0.14, 0.2),
            (0.2, -0.75, 0.18, 0.1),
        ] {
            rig.ellipse(x, y, rx, ry, 0.4, alpha(rgb(150, 92, 58), 0.55));
        }
    }

    // --- Eyes ---
    let bl = blink(t, a.seed);
    let open = match a.mood {
        Mood::Aim => 0.55,
        Mood::Yell => 1.1,
        _ => 1.0,
    } * (1.0 - bl);
    for sx in [-1.0f32, 1.0] {
        let (ex, ey) = (sx * 0.33, 0.2);
        match a.mood {
            Mood::Dizzy => {
                rig.ellipse(ex, ey, 0.2 + ow * 0.7, 0.21 + ow * 0.7, 0.0, pal.line);
                rig.ellipse(ex, ey, 0.2, 0.21, 0.0, WHITE);
                let spin = t * 9.0 * sx;
                let spiral: Vec<P> = (0..24)
                    .map(|i| {
                        let k = i as f32 / 23.0;
                        let th = spin + k * TAU * 1.6;
                        (ex + th.cos() * 0.16 * k, ey + th.sin() * 0.16 * k)
                    })
                    .collect();
                rig.stroke(&spiral, 0.035, pal.line);
            }
            Mood::Cheer => {
                let arc = bez(
                    (ex - 0.15, ey - 0.03),
                    (ex, ey + 0.17),
                    (ex + 0.15, ey - 0.03),
                    8,
                );
                rig.stroke(&arc, 0.07, pal.line);
            }
            _ if open < 0.15 => {
                rig.stroke(
                    &[(ex - 0.17, ey - 0.02), (ex + 0.17, ey - 0.02)],
                    0.06,
                    pal.line,
                );
            }
            _ => {
                let ry = 0.23 * open;
                rig.ellipse(ex, ey, 0.2 + ow * 0.7, ry + ow * 0.7, 0.0, pal.line);
                rig.ellipse(ex, ey, 0.2, ry, 0.0, WHITE);
                let pr = 0.1_f32.min(ry * 0.8);
                let (px, py) = (ex + look.0, ey + look.1 * open);
                rig.circle(px, py, pr, rgb(30, 20, 25));
                rig.circle(px - 0.035, py + 0.04, pr * 0.35, WHITE);
            }
        }
    }

    // --- Brows: always furious, livelier when yelling ---
    let jitter = if a.mood == Mood::Yell {
        (t * 30.0).sin() * 0.03
    } else {
        0.0
    };
    let (outer_y, inner_y) = match a.mood {
        Mood::Aim => (0.55, 0.3),
        Mood::Cheer => (0.62, 0.5),
        Mood::Dizzy => (0.52, 0.48),
        _ => (0.55 + jitter, 0.36 - jitter),
    };
    let brow_w = if kind == PigKind::Hog { 0.13 } else { 0.1 };
    for sx in [-1.0, 1.0] {
        let pts = bez(
            (sx * 0.6, outer_y),
            (sx * 0.35, outer_y + 0.02),
            (sx * 0.1, inner_y),
            6,
        );
        rig.stroke(&pts, brow_w, pal.line);
    }
    if kind == PigKind::Hog {
        rig.stroke(&[(-0.1, inner_y), (0.1, inner_y)], brow_w, pal.line);
    }

    // --- Snout with huffing nostrils ---
    let huff = 1.0 + 0.25 * (t * 2.6 + hash(a.seed, 9) * 6.0).sin().max(0.0).powi(3);
    let snout_y = -0.17;
    rig.blob(
        0.0,
        snout_y,
        0.36,
        0.26,
        0.0,
        mix(pal.light, pal.base, 0.3),
        pal.dark,
        Some((pal.line, ow * 0.8)),
    );
    for sx in [-1.0, 1.0] {
        rig.ellipse(
            sx * 0.13,
            snout_y,
            0.055 * huff,
            0.085 * huff,
            0.0,
            alpha(pal.line, 0.9),
        );
    }

    // --- Mouth ---
    let my = -0.58;
    match a.mood {
        Mood::Yell | Mood::Cheer => {
            let gape = if a.mood == Mood::Yell {
                0.15 + 0.04 * (t * 28.0).sin()
            } else {
                0.13
            };
            let mouth: Vec<P> = (0..=14)
                .map(|i| {
                    let th = PI + i as f32 / 14.0 * PI;
                    (th.cos() * 0.22, my + 0.05 + th.sin() * gape)
                })
                .collect();
            rig.poly(&mouth, rgb(90, 20, 34), Some((pal.line, 0.04)));
            rig.ellipse(
                0.0,
                my + 0.05 - gape * 0.6,
                0.12,
                gape * 0.35,
                0.0,
                rgb(232, 96, 116),
            );
        }
        Mood::Aim => {
            // Gritted teeth
            rig.poly(
                &[
                    (-0.2, my - 0.05),
                    (0.2, my - 0.05),
                    (0.2, my + 0.06),
                    (-0.2, my + 0.06),
                ],
                WHITE,
                Some((pal.line, 0.035)),
            );
            for x in [-0.07, 0.07] {
                rig.stroke(&[(x, my - 0.05), (x, my + 0.06)], 0.025, pal.line);
            }
            rig.stroke(&[(-0.2, my + 0.005), (0.2, my + 0.005)], 0.02, pal.line);
        }
        Mood::Dizzy => {
            let pts: Vec<P> = (0..=12)
                .map(|i| {
                    let x = -0.2 + i as f32 / 12.0 * 0.4;
                    (x, my + 0.03 * (x * 40.0 + t * 8.0).sin())
                })
                .collect();
            rig.stroke(&pts, 0.045, pal.line);
        }
        _ => {
            let pts = bez((-0.22, my - 0.02), (0.0, my + 0.1), (0.22, my - 0.02), 8);
            rig.stroke(&pts, 0.055, pal.line);
        }
    }

    // --- Per-kind gear ---
    // On Thanksgiving a pilgrim hat replaces whatever grows on top.
    let lift = if pilgrim {
        pilgrim_hat(&rig, pal.line);
        0.66
    } else {
        0.0
    };
    match kind {
        _ if pilgrim && matches!(kind, PigKind::Grunt | PigKind::Litter | PigKind::Piglet) => {}
        PigKind::Grunt => {
            // A defiant curl of hair.
            let curl: Vec<P> = (0..18)
                .map(|i| {
                    let k = i as f32 / 17.0;
                    let th = -0.5 + k * TAU * 0.9;
                    let r = 0.14 * (1.0 - k * 0.5);
                    (0.05 + th.cos() * r, 1.08 + k * 0.08 + th.sin() * r)
                })
                .collect();
            rig.stroke(&curl, 0.12, pal.line);
            rig.stroke(&curl, 0.06, pal.base);
        }
        PigKind::Dasher => {
            let band = bez((-0.93, 0.48), (0.0, 0.62), (0.93, 0.48), 10);
            rig.stroke(&band, 0.2, pal.line);
            rig.stroke(&band, 0.14, rgb(222, 44, 50));
            rig.circle(back * 0.86, 0.5, 0.1, rgb(190, 30, 40));
        }
        PigKind::Boomer => {
            let fuse = bez(
                (0.05, 0.95 + lift),
                (0.05, 1.3 + lift),
                (0.35, 1.38 + lift),
                8,
            );
            rig.stroke(&fuse, 0.1, pal.line);
            rig.stroke(&fuse, 0.05, rgb(170, 150, 120));
            let (sx, sy) = (0.37, 1.4 + lift);
            let flick = 0.8 + 0.3 * (t * 37.0).sin() + a.glow * 0.6;
            rig.circle(sx, sy, 0.2 * flick, rgba(255, 170, 40, 0.35));
            for i in 0..6 {
                let th = i as f32 / 6.0 * TAU + t * 8.0;
                let len = 0.16 * flick * (0.7 + 0.3 * (t * 20.0 + i as f32).sin());
                rig.stroke(
                    &[(sx, sy), (sx + th.cos() * len, sy + th.sin() * len)],
                    0.035,
                    rgb(255, 210, 80),
                );
            }
            rig.circle(sx, sy, 0.06 * flick, rgb(255, 255, 220));
        }
        PigKind::Hog => {
            for sx in [-1.0, 1.0] {
                let tusk = bez(
                    (sx * 0.22, my - 0.02),
                    (sx * 0.4, my - 0.02),
                    (sx * 0.36, my + 0.32),
                    6,
                );
                let mut pts = tusk.clone();
                pts.push((sx * 0.3, my));
                rig.poly(&pts, rgb(252, 246, 226), Some((pal.line, 0.035)));
            }
        }
        PigKind::Litter => {
            // Three-tuft topknot, one per piglet inside.
            for (i, x) in [-0.18f32, 0.0, 0.18].into_iter().enumerate() {
                let sway = (t * 4.0 + i as f32).sin() * 0.04;
                let pts = bez((x, 0.95), (x + 0.1 + sway, 1.15), (x + sway, 1.3), 6);
                rig.stroke(&pts, 0.1, pal.line);
                rig.stroke(&pts, 0.05, pal.dark);
            }
        }
        PigKind::Piglet => {
            // A tiny curl, like a smaller Grunt.
            let curl = bez((0.0, 0.98), (0.2, 1.15), (0.05, 1.22), 6);
            rig.stroke(&curl, 0.12, pal.line);
            rig.stroke(&curl, 0.06, pal.base);
        }
        PigKind::Thumper => {
            // Mohawk (under the hat on Thanksgiving)
            for i in (0..5).filter(|_| !pilgrim) {
                let x = -0.32 + i as f32 * 0.16;
                let h = 0.32 - (x.abs() * 0.4);
                rig.poly(
                    &[(x - 0.09, 0.9), (x + 0.02, 0.92 + h), (x + 0.09, 0.9)],
                    rgb(40, 30, 36),
                    Some((pal.line, 0.02)),
                );
            }
            // Wrestler's mask stripes across the eyes
            for sx in [-1.0, 1.0] {
                rig.stroke(
                    &[(sx * 0.95, 0.3), (sx * 0.58, 0.25)],
                    0.09,
                    alpha(pal.line, 0.8),
                );
            }
        }
        PigKind::Puffer => {}
    }
}

struct WolfPalette {
    light: Color,
    base: Color,
    dark: Color,
    line: Color,
    cream: Color,
    cream_dark: Color,
    iris: (Color, Color),
}

fn wolf_palette(kind: WolfKind, hurt: f32) -> WolfPalette {
    let red = rgb(240, 110, 110);
    let (light, base, dark, iris) = match kind {
        WolfKind::Pup => (
            rgb(232, 220, 208),
            rgb(170, 150, 136),
            rgb(124, 106, 94),
            (rgb(200, 240, 255), rgb(90, 170, 230)),
        ),
        WolfKind::Boss => (
            rgb(150, 150, 172),
            rgb(74, 76, 96),
            rgb(48, 50, 64),
            (rgb(255, 210, 150), rgb(240, 90, 40)),
        ),
        WolfKind::Wolf | WolfKind::Helmet => (
            rgb(206, 210, 224),
            rgb(128, 134, 158),
            rgb(88, 92, 114),
            (rgb(255, 245, 190), rgb(250, 205, 70)),
        ),
    };
    WolfPalette {
        light: mix(light, red, hurt * 0.4),
        base: mix(base, red, hurt * 0.5),
        dark,
        line: rgb(38, 40, 56),
        cream: rgb(244, 236, 220),
        cream_dark: rgb(208, 196, 176),
        iris,
    }
}

/// The Helmet wolf's cooking pot, until it's knocked about.
pub(crate) fn pot(rig: &Rig, line: Color) {
    let metal_light = rgb(214, 218, 226);
    let metal = rgb(140, 146, 160);
    let dome: Vec<P> = (0..=16)
        .map(|i| {
            let th = i as f32 / 16.0 * PI;
            (th.cos() * 0.98, 0.62 + th.sin() * 0.72)
        })
        .collect();
    rig.poly(&dome, metal, Some((line, 0.06)));
    rig.ellipse(-0.3, 1.05, 0.28, 0.14, 0.4, alpha(metal_light, 0.8));
    // Rim and handle
    rig.poly(
        &[(-1.12, 0.56), (1.12, 0.56), (1.12, 0.7), (-1.12, 0.7)],
        metal,
        Some((line, 0.04)),
    );
    rig.poly(
        &[(0.95, 0.9), (1.45, 0.98), (1.45, 1.08), (0.95, 1.06)],
        rgb(60, 50, 50),
        Some((line, 0.03)),
    );
    for x in [-0.6, 0.0, 0.6] {
        rig.circle(x, 0.63, 0.04, line);
    }
}

/// A smug wolf. `hp` is 0..1, `hurt` is the recent-hit flash. On
/// Thanksgiving, a smug turkey instead.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub fn wolf(
    view: &View,
    kind: WolfKind,
    pos: V2,
    angle: f32,
    radius: f32,
    hp: f32,
    hurt: f32,
    a: &Anim,
) {
    if crate::holiday::thanksgiving() {
        crate::costume::turkey(view, kind, pos, angle, radius, hp, hurt, a);
        return;
    }
    let t = a.t;
    let bounce = match a.mood {
        Mood::Laugh => 0.07 * (t * 22.0).sin(),
        Mood::Hurt => -0.08 * hurt,
        _ => 0.0,
    };
    let shake = if a.mood == Mood::Worried {
        (t * 40.0).sin() * 0.012
    } else {
        0.0
    };
    let rig = rig_for(view, pos + V2::new(shake, 0.0), angle, radius, a, bounce);
    let pal = wolf_palette(kind, hurt);
    let ow = 0.07;
    let look = gaze(&rig, a.look, 0.06);

    // --- Ears: occasional twitch, droop when worried ---
    let twitch_period = 3.0 + hash(a.seed, 11) * 3.0;
    let tp = (t + hash(a.seed, 12) * 9.0) % twitch_period;
    let twitch = if tp < 0.3 {
        (tp * 40.0).sin() * 0.25
    } else {
        0.0
    };
    let droop = match a.mood {
        Mood::Worried | Mood::Hurt => 0.45,
        Mood::Laugh => 0.1 * (t * 22.0).sin(),
        _ => 0.0,
    };
    for sx in [-1.0f32, 1.0] {
        let tw = if sx > 0.0 { twitch } else { 0.0 };
        let ang = PI / 2.0 - sx * (0.32 + droop) + tw;
        let base = (sx * 0.52, 0.62);
        let ear_len = if kind == WolfKind::Pup { 0.6 } else { 0.85 };
        let pts = ear(base, ang, ear_len, 0.26);
        rig.poly(&pts, pal.base, Some((pal.line, ow)));
        let inner = shrink(
            &pts,
            0.55,
            (base.0 + ang.cos() * 0.25, base.1 + ang.sin() * 0.25),
        );
        rig.poly(&inner, rgb(228, 160, 172), None);
    }

    // --- Head with shaggy cheek fur and a tuft on top ---
    let head = |th: f32| {
        let s = th.sin();
        let mut rr = 1.0;
        if s < -0.05 {
            let tri = ((th * 9.0 / PI) % 1.0 - 0.5).abs() * 2.0;
            rr += 0.14 * th.cos().abs() * tri;
        }
        if s > 0.9 {
            let tri = ((th * 14.0 / PI) % 1.0 - 0.5).abs() * 2.0;
            rr += 0.12 * tri * (s - 0.9) * 10.0;
        }
        rr
    };
    rig.radial((0.0, 0.0), head, ow, pal.line, pal.line, (0.0, 0.0));
    rig.radial((0.0, 0.0), head, 0.0, pal.light, pal.base, (-0.3, 0.45));
    rig.ellipse(0.0, -0.8, 0.55, 0.14, 0.0, alpha(pal.dark, 0.4));
    rig.ellipse(-0.45, 0.52, 0.18, 0.1, 0.7, rgba(255, 255, 255, 0.35));

    // Cream face mask
    let mask = |th: f32| {
        let s = th.sin();
        0.62 + 0.08 * s.min(0.0).abs()
    };
    rig.radial(
        (0.0, -0.32),
        mask,
        0.0,
        pal.cream,
        pal.cream_dark,
        (-0.1, 0.15),
    );

    // --- Eyes ---
    let bl = blink(t, a.seed);
    for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (ex, ey) = (sx * 0.34, 0.24);
        match a.mood {
            Mood::Hurt => {
                let pts = [
                    (ex - sx * 0.14, ey + 0.1),
                    (ex + sx * 0.08, ey),
                    (ex - sx * 0.14, ey - 0.1),
                ];
                rig.stroke(&pts, 0.07, pal.line);
                continue;
            }
            Mood::Laugh => {
                let arc = bez(
                    (ex - 0.15, ey - 0.04),
                    (ex, ey + 0.16),
                    (ex + 0.15, ey - 0.04),
                    8,
                );
                rig.stroke(&arc, 0.07, pal.line);
                continue;
            }
            _ => {}
        }
        let worried = a.mood == Mood::Worried;
        let ry = if worried { 0.24 } else { 0.2 };
        rig.ellipse(ex, ey, 0.19 + ow * 0.7, ry + ow * 0.7, 0.0, pal.line);
        rig.blob(ex, ey, 0.19, ry, 0.0, pal.iris.0, pal.iris.1, None);
        let (pw, ph) = if worried {
            (0.045, 0.07)
        } else {
            (0.065, 0.13)
        };
        let (px, py) = (ex + look.0, ey + look.1);
        rig.ellipse(px, py, pw, ph, 0.0, rgb(20, 18, 24));
        rig.circle(px - 0.03, py + 0.05, 0.03, WHITE);

        // Black eye once badly beaten.
        if hp < 0.35 && i == 0 {
            let bruise: Vec<P> = (0..=20)
                .map(|k| {
                    let th = k as f32 / 20.0 * TAU;
                    (ex + th.cos() * 0.25, ey + th.sin() * 0.27)
                })
                .collect();
            rig.stroke(&bruise, 0.06, rgba(110, 60, 140, 0.7));
        }

        // Heavy smug lids (or a blink).
        let lid = if worried {
            bl
        } else {
            (if sx > 0.0 { 0.5_f32 } else { 0.4 }).max(bl)
        };
        if lid > 0.02 {
            let top = ey + ry + 0.02;
            let bottom = top - (ry * 2.0 + 0.04) * lid;
            let lid_pts: Vec<P> = (0..=12)
                .map(|k| {
                    let th = k as f32 / 12.0 * PI;
                    (
                        ex + th.cos() * (0.19 + ow * 0.7),
                        bottom + th.sin() * (top - bottom),
                    )
                })
                .collect();
            rig.poly(&lid_pts, pal.base, None);
            rig.stroke(&[(ex - 0.21, bottom), (ex + 0.21, bottom)], 0.05, pal.line);
        }
    }

    // --- Brows ---
    for sx in [-1.0f32, 1.0] {
        let (outer, inner) = match a.mood {
            Mood::Worried => (0.5, 0.62),
            Mood::Hurt => (0.58, 0.4),
            Mood::Laugh => (0.58, 0.55),
            // Smug: one brow cocked high.
            _ if sx > 0.0 => (0.66 + 0.02 * (t * 1.5).sin(), 0.56),
            _ => (0.5, 0.5),
        };
        let pts = bez(
            (sx * 0.56, outer),
            (sx * 0.34, outer + 0.06),
            (sx * 0.12, inner),
            6,
        );
        rig.stroke(&pts, 0.09, pal.line);
    }

    // --- Nose ---
    rig.ellipse(0.0, -0.1, 0.2, 0.13, 0.0, pal.line);
    rig.blob(
        0.0,
        -0.1,
        0.17,
        0.1,
        0.0,
        rgb(90, 84, 100),
        rgb(30, 28, 36),
        None,
    );
    rig.ellipse(-0.05, -0.06, 0.05, 0.025, 0.0, rgba(255, 255, 255, 0.6));
    rig.stroke(&[(0.0, -0.21), (0.0, -0.32)], 0.04, pal.line);

    // --- Mouth ---
    let my = -0.36;
    match a.mood {
        Mood::Laugh => {
            let gape = 0.26 + 0.05 * (t * 22.0).sin();
            let mouth: Vec<P> = (0..=16)
                .map(|i| {
                    let th = PI + i as f32 / 16.0 * PI;
                    (th.cos() * 0.34, my + th.sin() * gape)
                })
                .collect();
            rig.poly(&mouth, rgb(110, 26, 40), Some((pal.line, 0.04)));
            rig.ellipse(
                0.0,
                my - gape * 0.7,
                0.16,
                gape * 0.28,
                0.0,
                rgb(236, 110, 128),
            );
            for sx in [-1.0, 1.0] {
                rig.poly(
                    &[(sx * 0.26, my), (sx * 0.16, my), (sx * 0.21, my - 0.11)],
                    WHITE,
                    None,
                );
            }
        }
        Mood::Worried => {
            let pts: Vec<P> = (0..=12)
                .map(|i| {
                    let x = -0.24 + i as f32 / 12.0 * 0.48;
                    (x, my - 0.04 + 0.025 * (x * 45.0 + t * 12.0).sin())
                })
                .collect();
            rig.stroke(&pts, 0.045, pal.line);
            // Sweat drop sliding down.
            let k = (t * 0.8 + hash(a.seed, 4)) % 1.0;
            let (dx, dy) = (0.82, 0.55 - k * 0.5);
            rig.poly(
                &[
                    (dx, dy + 0.14),
                    (dx - 0.08, dy),
                    (dx, dy - 0.07),
                    (dx + 0.08, dy),
                ],
                alpha(rgb(130, 200, 255), 1.0 - k * 0.6),
                Some((alpha(rgb(60, 120, 200), 1.0 - k * 0.6), 0.02)),
            );
        }
        Mood::Hurt => {
            rig.poly(
                &[
                    (-0.24, my - 0.1),
                    (0.24, my - 0.1),
                    (0.24, my + 0.02),
                    (-0.24, my + 0.02),
                ],
                WHITE,
                Some((pal.line, 0.035)),
            );
            for x in [-0.12, 0.0, 0.12] {
                rig.stroke(&[(x, my - 0.1), (x, my + 0.02)], 0.022, pal.line);
            }
        }
        _ => {
            // Smirk with a fang.
            let smirk = bez((-0.3, my + 0.02), (0.02, my - 0.12), (0.34, my + 0.06), 10);
            rig.stroke(&smirk, 0.05, pal.line);
            rig.poly(
                &[(0.1, my - 0.07), (0.2, my - 0.04), (0.16, my - 0.18)],
                WHITE,
                Some((pal.line, 0.02)),
            );
        }
    }

    // Bandage once hurt.
    if hp < 0.6 {
        let (bx, by) = (-0.45, 0.72);
        for rot in [0.6f32, -0.6] {
            let (s, c) = rot.sin_cos();
            let (hw, hh) = (0.28, 0.08);
            let corners: Vec<P> = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
                .iter()
                .map(|&(x, y)| (bx + x * c - y * s, by + x * s + y * c))
                .collect();
            rig.poly(
                &corners,
                rgb(250, 240, 215),
                Some((rgb(170, 150, 120), 0.02)),
            );
        }
    }

    match kind {
        WolfKind::Helmet if hp > 0.5 => pot(&rig, pal.line),
        WolfKind::Boss => {
            // A scar over one eye and a gold earring.
            let scar = [(0.16, 0.6), (0.36, 0.25), (0.56, -0.08)];
            rig.stroke(&scar, 0.07, rgb(214, 120, 132));
            for (x, y) in [(0.22, 0.48), (0.34, 0.3), (0.47, 0.1)] {
                rig.stroke(
                    &[(x - 0.07, y - 0.04), (x + 0.07, y + 0.04)],
                    0.035,
                    rgb(214, 120, 132),
                );
            }
            let earring: Vec<P> = (0..=16)
                .map(|i| {
                    let th = i as f32 / 16.0 * TAU;
                    (-0.86 + th.cos() * 0.1, 0.72 + th.sin() * 0.1)
                })
                .collect();
            rig.stroke(&earring, 0.05, rgb(240, 196, 60));
        }
        _ => {}
    }
}

/// A tiny four-pointed twinkle.
pub(crate) fn twinkle(rig: &Rig, x: f32, y: f32, size: f32, col: Color) {
    rig.ellipse(x, y, size, size * 0.22, 0.0, col);
    rig.ellipse(x, y, size * 0.22, size, 0.0, col);
}

/// A little fairy, arms raised to hold up whatever rests on her. `r` is her
/// collider's radius. `startled` is set while she flees.
/// A soft pool of light: an ellipse (in a rig's local units) that brightens
/// whatever is behind it, fading to nothing at its edge.
fn light(rig: &Rig, center: P, rx: f32, ry: f32, col: Color) {
    let edge = Rig::ellipse_ring(center.0, center.1, rx, ry, 0.0);
    crate::gfx::glow(|| rig.fan(center, &edge, col, alpha(col, 0.0)));
}

/// The light a fairy gives off: brighter the darker the scene.
fn fairy_light(view: &View, pos: V2, r: f32, a: &Anim) {
    // She shines, more brightly the darker the scene.
    let shine = crate::render::glow_strength();
    let flicker = 0.85 + 0.15 * ((a.t + hash(a.seed, 9) * 10.0) * 3.0).sin();
    let halo = Rig::new(*view, pos, 0.0, r);
    light(
        &halo,
        (0.0, 0.1),
        2.8,
        2.8,
        rgba(255, 228, 150, 0.5 * shine * flicker),
    );
    light(
        &halo,
        (0.0, 0.1),
        1.3,
        1.3,
        rgba(255, 246, 210, 0.45 * shine),
    );
}

#[allow(clippy::too_many_lines)]
pub fn fairy(view: &View, pos: V2, r: f32, a: &Anim, startled: bool) {
    fairy_light(view, pos, r, a);
    if crate::holiday::thanksgiving() {
        crate::costume::cherub(view, pos, r, a, startled);
        return;
    }
    let t = a.t + hash(a.seed, 9) * 10.0;
    let rig = Rig::new(*view, pos, 0.0, r);
    let line = rgb(92, 52, 120);
    let skin = rgb(255, 222, 200);
    let hair = rgb(255, 196, 84);
    let ow = 0.05;

    // Soft glow.
    let glow: Vec<P> = (0..32)
        .map(|i| {
            let th = i as f32 / 32.0 * TAU;
            (th.cos() * 1.35, th.sin() * 1.35)
        })
        .collect();
    let pulse = 0.28 + 0.08 * (t * 3.0).sin();
    rig.fan(
        (0.0, 0.1),
        &glow,
        rgba(255, 250, 210, pulse),
        rgba(255, 250, 210, 0.0),
    );

    // Wings: a fast shimmering flutter.
    let speed = if startled { 46.0 } else { 26.0 };
    let flap = 0.55 + 0.45 * (t * speed).sin().abs();
    let wing = rgba(205, 238, 255, 0.6);
    let edge = rgba(120, 172, 226, 0.9);
    for sx in [-1.0f32, 1.0] {
        let (rx, ry) = (0.6 * flap, 0.36);
        rig.blob(
            sx * (0.12 + rx * 0.9),
            0.28,
            rx,
            ry,
            sx * 0.5,
            rgba(255, 255, 255, 0.75),
            wing,
            Some((edge, 0.035)),
        );
        let (rx, ry) = (0.42 * flap, 0.24);
        rig.blob(
            sx * (0.1 + rx * 0.9),
            -0.22,
            rx,
            ry,
            -sx * 0.45,
            rgba(255, 255, 255, 0.75),
            wing,
            Some((edge, 0.035)),
        );
    }

    // Arms: raised overhead to hold things up; flung wide when startled.
    for sx in [-1.0f32, 1.0] {
        let pts = if startled {
            bez((sx * 0.18, 0.02), (sx * 0.5, 0.15), (sx * 0.72, 0.42), 6)
        } else {
            bez((sx * 0.18, 0.02), (sx * 0.5, 0.45), (sx * 0.34, 0.88), 6)
        };
        rig.stroke(&pts, 0.17, line);
        rig.stroke(&pts, 0.09, skin);
        let (hx, hy) = pts[pts.len() - 1];
        rig.blob(
            hx,
            hy,
            0.085,
            0.085,
            0.0,
            rgb(255, 238, 224),
            skin,
            Some((line, 0.035)),
        );
    }

    // Legs, dangling and kicking a little.
    for sx in [-1.0f32, 1.0] {
        let kick = (t * 5.0 + sx * 1.3).sin() * 0.05;
        let pts = [(sx * 0.12, -0.6), (sx * 0.14 + kick, -0.9)];
        rig.stroke(&pts, 0.12, line);
        rig.stroke(&pts, 0.06, skin);
    }

    // Petal dress.
    let dress = rgb(200, 146, 234);
    let dress_light = rgb(236, 206, 255);
    let mut hem: Vec<P> = vec![(-0.17, 0.06)];
    for i in 0..=8 {
        let u = i as f32 / 8.0;
        let x = -0.5 + u;
        let dip = if i % 2 == 0 { -0.72 } else { -0.62 };
        hem.push((x, dip + 0.02 * (t * 4.0 + u * 6.0).sin()));
    }
    hem.push((0.17, 0.06));
    rig.poly(&hem, dress, Some((line, ow)));
    rig.ellipse(-0.08, -0.25, 0.1, 0.25, 0.2, alpha(dress_light, 0.5));

    // Head with a golden bun.
    rig.blob(
        0.0,
        0.62,
        0.14,
        0.13,
        0.0,
        rgb(255, 226, 140),
        hair,
        Some((line, ow)),
    );
    rig.blob(
        0.0,
        0.33,
        0.34,
        0.31,
        0.0,
        rgb(255, 226, 140),
        hair,
        Some((line, ow)),
    );
    rig.blob(0.0, 0.29, 0.27, 0.26, 0.0, rgb(255, 240, 228), skin, None);
    // Fringe.
    rig.poly(
        &[
            (-0.28, 0.42),
            (-0.1, 0.5),
            (0.1, 0.5),
            (0.28, 0.42),
            (0.0, 0.6),
        ],
        hair,
        None,
    );

    // Face.
    let look = gaze(&rig, a.look, 0.04);
    if startled {
        for sx in [-1.0f32, 1.0] {
            rig.circle(sx * 0.1, 0.3, 0.065, WHITE);
            rig.circle(sx * 0.1 + look.0, 0.3 + look.1, 0.035, line);
        }
        rig.ellipse(0.0, 0.16, 0.04, 0.05, 0.0, line);
    } else {
        let shut = blink(a.t, a.seed);
        for sx in [-1.0f32, 1.0] {
            rig.ellipse(
                sx * 0.1 + look.0,
                0.3 + look.1,
                0.035,
                0.05 * (1.0 - shut).max(0.15),
                0.0,
                line,
            );
        }
        let smile = bez((-0.07, 0.19), (0.0, 0.14), (0.07, 0.19), 5);
        rig.stroke(&smile, 0.03, line);
    }
    for sx in [-1.0f32, 1.0] {
        rig.circle(sx * 0.17, 0.22, 0.05, rgba(255, 130, 160, 0.45));
    }

    // Twinkles circling her.
    for i in 0..3 {
        let th = t * 1.4 + i as f32 * TAU / 3.0;
        let tw = 0.5 + 0.5 * (t * 6.0 + i as f32 * 2.0).sin();
        twinkle(
            &rig,
            th.cos() * 1.1,
            0.1 + th.sin() * 0.8,
            0.08 + 0.08 * tw,
            rgba(255, 250, 200, 0.5 + 0.5 * tw),
        );
    }
}

/// A wide white pilgrim collar, peeking out under the chin.
fn pilgrim_collar(rig: &Rig, line: Color) {
    let white = rgb(250, 248, 240);
    for sx in [-1.0f32, 1.0] {
        let flap = [
            (0.0, -0.62),
            (sx * 0.78, -0.66),
            (sx * 0.66, -1.02),
            (sx * 0.08, -1.12),
        ];
        rig.poly(&flap, white, Some((line, 0.05)));
    }
}

/// A tall black pilgrim hat with a gold buckle, sitting on top of the head.
fn pilgrim_hat(rig: &Rig, line: Color) {
    let felt = rgb(54, 48, 56);
    let felt_light = rgb(88, 80, 92);
    let gold = rgb(240, 196, 70);
    rig.ellipse(0.0, 0.86, 0.84, 0.15, 0.0, line);
    rig.ellipse(0.0, 0.86, 0.78, 0.11, 0.0, felt);
    rig.poly(
        &[(-0.47, 0.86), (0.47, 0.86), (0.4, 1.56), (-0.4, 1.56)],
        felt,
        Some((line, 0.05)),
    );
    rig.ellipse(0.0, 1.56, 0.4, 0.07, 0.0, felt_light);
    rig.poly(
        &[(-0.34, 0.95), (-0.27, 0.95), (-0.22, 1.5), (-0.28, 1.5)],
        alpha(felt_light, 0.7),
        None,
    );
    // Band and buckle.
    rig.poly(
        &[(-0.465, 0.9), (0.465, 0.9), (0.45, 1.08), (-0.45, 1.08)],
        rgb(28, 24, 30),
        None,
    );
    rig.stroke(
        &[
            (-0.13, 0.89),
            (0.13, 0.89),
            (0.13, 1.09),
            (-0.13, 1.09),
            (-0.13, 0.89),
        ],
        0.06,
        gold,
    );
}

/// Both ends of a portal pair.
pub fn portal(view: &View, portal: &Portal, t: f32) {
    for (k, end) in portal.ends.iter().enumerate() {
        portal_end(view, *end, portal.r, portal.hue, t + k as f32 * 1.7);
    }
}

/// One end of a portal: an upright glowing oval (at angle 0) with a swirl
/// inside, in the pair's color. `r` is half the opening's height.
pub fn portal_end(view: &View, end: PortalEnd, r: f32, hue: u16, t: f32) {
    let hue = f32::from(hue);
    let rig = Rig::new(*view, end.pos, end.angle, r);
    let bright = hsv(hue, 0.75, 1.0);
    let deep = hsv(hue, 0.9, 0.45);
    let pale = hsv(hue, 0.25, 1.0);
    let (rx, ry) = (0.42, 1.0);

    // Glow around it, gently pulsing, and light spilling out of it (brighter
    // the darker the scene).
    let glow = Rig::ellipse_ring(0.0, 0.0, rx + 0.4, ry + 0.3, 0.0);
    let pulse = 0.35 + 0.1 * (t * 2.5).sin();
    rig.fan((0.0, 0.0), &glow, alpha(bright, pulse), alpha(bright, 0.0));
    let shine = crate::render::glow_strength() * (0.85 + 0.15 * (t * 2.5).sin());
    light(
        &rig,
        (0.0, 0.0),
        rx + 1.6,
        ry + 1.0,
        alpha(bright, 0.55 * shine),
    );

    // The swirling hole: bright in the middle, deep at the edge.
    let hole = Rig::ellipse_ring(0.0, 0.0, rx, ry, 0.0);
    rig.fan((0.0, 0.0), &hole, alpha(pale, 0.95), alpha(deep, 0.95));
    for arm in 0..3 {
        let spin = t * 2.2 + arm as f32 / 3.0 * TAU;
        let pts: Vec<P> = (0..14)
            .map(|i| {
                let f = i as f32 / 13.0;
                let a = spin + f * 3.6;
                let d = 0.12 + f * 0.82;
                (a.cos() * d * rx, a.sin() * d * ry)
            })
            .collect();
        rig.stroke(&pts, 0.07, alpha(pale, 0.7));
    }

    // Rim, with the solid knobs at each tip.
    let mut rim = Rig::ellipse_ring(0.0, 0.0, rx, ry, 0.0);
    rim.push(rim[0]);
    rig.stroke(&rim, 0.2, alpha(deep, 0.9));
    rig.stroke(&rim, 0.11, bright);
    light(
        &rig,
        (0.0, 0.0),
        rx,
        ry,
        alpha(pale, 0.35 * crate::render::glow_strength()),
    );
    let knob = 0.15 / r.max(0.1);
    for sy in [-1.0f32, 1.0] {
        rig.blob(
            0.0,
            sy * ry,
            knob,
            knob,
            0.0,
            pale,
            bright,
            Some((deep, 0.04)),
        );
    }

    // Sparkles drifting off the rim.
    for i in 0..5 {
        let seed = i as f32 * 1.37;
        let life = (t * 0.6 + seed * 0.29) % 1.0;
        let a = seed * 2.3 + t * 0.3;
        let d = 1.0 + life * 0.5;
        let (x, y) = (a.cos() * rx * d, a.sin() * ry * d);
        let size = 0.07 * (1.0 - life);
        rig.circle(x, y, size, alpha(pale, 1.0 - life));
    }
}
