//! All drawing. Everything is procedural shapes — no image assets.

use macroquad::prelude::*;

use crate::sim::{self, Ent, Game, Kind, Material, Phase, PigKind, Shape, V2};

/// Maps world meters (y up) to screen pixels (y down).
#[derive(Clone, Copy)]
pub struct View {
    pub s: f32,
    pub ox: f32,
    pub oy: f32,
}

pub const VIEW_W: f32 = 42.0;
pub const VIEW_H: f32 = 22.0;
const VIEW_X0: f32 = -1.0;
const VIEW_Y0: f32 = -1.5;

impl View {
    pub fn fit(shake: Vec2) -> Self {
        let (sw, sh) = (screen_width(), screen_height());
        let s = (sw / VIEW_W).min(sh / VIEW_H);
        let ox = (sw - VIEW_W * s) / 2.0 - VIEW_X0 * s + shake.x;
        // Anchor the bottom of the view to the bottom of the screen.
        let oy = sh + VIEW_Y0 * s + shake.y;
        View { s, ox, oy }
    }
    pub fn p(&self, x: f32, y: f32) -> Vec2 {
        vec2(self.ox + x * self.s, self.oy - y * self.s)
    }
    pub fn v(&self, v: V2) -> Vec2 {
        self.p(v.x, v.y)
    }
}

/// Local frame of a rotated body, for drawing details in body space.
struct Local {
    view: View,
    c: V2,
    cos: f32,
    sin: f32,
}

impl Local {
    fn new(view: View, c: V2, angle: f32) -> Self {
        Local {
            view,
            c,
            cos: angle.cos(),
            sin: angle.sin(),
        }
    }
    fn p(&self, x: f32, y: f32) -> Vec2 {
        let wx = self.c.x + x * self.cos - y * self.sin;
        let wy = self.c.y + x * self.sin + y * self.cos;
        self.view.p(wx, wy)
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, col: Color) {
        let (pa, pb) = (self.p(a.0, a.1), self.p(b.0, b.1));
        draw_line(pa.x, pa.y, pb.x, pb.y, w * self.view.s, col);
    }
    fn circle(&self, x: f32, y: f32, r: f32, col: Color) {
        let p = self.p(x, y);
        draw_circle(p.x, p.y, r * self.view.s, col);
    }
    fn tri(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Color) {
        draw_triangle(self.p(a.0, a.1), self.p(b.0, b.1), self.p(c.0, c.1), col);
    }
    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32, col: Color) {
        let p = self.p(x, y);
        let deg = -self.sin.atan2(self.cos).to_degrees();
        draw_ellipse(p.x, p.y, rx * self.view.s, ry * self.view.s, deg, col);
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn background(view: &View, time: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let top = rgb(110, 180, 235);
    let bottom = rgb(205, 235, 250);
    let strips = 24;
    for i in 0..strips {
        let t = i as f32 / strips as f32;
        draw_rectangle(
            0.0,
            sh * t,
            sw,
            sh / strips as f32 + 1.0,
            mix(top, bottom, t),
        );
    }
    // Sun
    let sun = view.p(36.0, 17.0);
    draw_circle(sun.x, sun.y, 2.2 * view.s, Color::new(1.0, 0.95, 0.7, 0.5));
    draw_circle(sun.x, sun.y, 1.6 * view.s, rgb(255, 240, 170));

    // Clouds drifting
    for (i, (y, sz, speed)) in [(15.0, 1.0, 0.3), (12.5, 0.7, 0.5), (17.5, 1.3, 0.2)]
        .iter()
        .enumerate()
    {
        let span = VIEW_W + 12.0;
        let x = ((i as f32 * 17.0 + time * speed) % span) - 6.0;
        let c = Color::new(1.0, 1.0, 1.0, 0.9);
        for (dx, dy, r) in [
            (0.0, 0.0, 1.0),
            (1.1, 0.3, 1.2),
            (2.3, 0.0, 0.9),
            (1.1, -0.3, 0.9),
        ] {
            let p = view.p(x + dx * sz, y + dy * sz);
            draw_circle(p.x, p.y, r * sz * view.s, c);
        }
    }

    // Rolling hills
    let far = rgb(140, 200, 130);
    let near = rgb(110, 180, 100);
    for (i, &(x, r)) in [(-2.0, 9.0), (12.0, 11.0), (28.0, 8.0), (42.0, 12.0)]
        .iter()
        .enumerate()
    {
        let p = view.p(x, -r * 0.55);
        draw_circle(p.x, p.y, r * view.s, if i % 2 == 0 { far } else { near });
    }

    // Ground
    let g = view.p(0.0, 0.0);
    draw_rectangle(0.0, g.y, sw, sh - g.y + 1.0, rgb(122, 86, 58));
    draw_rectangle(0.0, g.y, sw, 0.35 * view.s, rgb(96, 168, 72));
    draw_rectangle(
        0.0,
        g.y + 0.35 * view.s,
        sw,
        0.12 * view.s,
        rgb(78, 140, 60),
    );
}

fn material_colors(m: Material) -> (Color, Color) {
    match m {
        Material::Straw => (rgb(236, 205, 110), rgb(190, 150, 60)),
        Material::Wood => (rgb(180, 120, 70), rgb(110, 70, 40)),
        Material::Brick => (rgb(178, 82, 64), rgb(110, 50, 40)),
    }
}

pub fn block(view: &View, e: &Ent) {
    let Shape::Box { hw, hh } = e.shape else {
        return;
    };
    let l = Local::new(*view, e.pos, e.angle);
    let (fill, dark) = match e.kind {
        Kind::Block(m) => material_colors(m),
        _ => (rgb(130, 125, 120), rgb(90, 85, 80)),
    };
    let fill = mix(fill, WHITE, e.hurt_flash * 0.35);
    let c = view.v(e.pos);
    draw_rectangle_ex(
        c.x,
        c.y,
        hw * 2.0 * view.s,
        hh * 2.0 * view.s,
        DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation: -e.angle,
            color: fill,
        },
    );

    // Surface detail, laid out along the long axis.
    let long_x = hw >= hh;
    let thin = 0.04;
    match e.kind {
        Kind::Block(Material::Straw) => {
            let n = ((if long_x { hh } else { hw }) * 2.0 / 0.13) as i32;
            for i in 1..n {
                let f = -1.0 + 2.0 * i as f32 / n as f32;
                if long_x {
                    l.line((-hw, f * hh), (hw, f * hh), thin, dark);
                } else {
                    l.line((f * hw, -hh), (f * hw, hh), thin, dark);
                }
            }
        }
        Kind::Block(Material::Wood) => {
            for f in [-0.4, 0.15, 0.55] {
                if long_x {
                    l.line((-hw * 0.9, f * hh), (hw * 0.7, f * hh), thin, dark);
                } else {
                    l.line((f * hw, -hh * 0.9), (f * hw, hh * 0.7), thin, dark);
                }
            }
        }
        Kind::Block(Material::Brick) => {
            let (bw, bh) = (0.5, 0.25);
            let (ax, ay) = if long_x { (hw, hh) } else { (hh, hw) };
            let rows = ((ay * 2.0) / bh).round().max(1.0) as i32;
            for r in 0..rows {
                let y0 = -ay + r as f32 * (ay * 2.0 / rows as f32);
                if r > 0 {
                    let (a, b) = ((-ax, y0), (ax, y0));
                    if long_x {
                        l.line(a, b, thin, dark)
                    } else {
                        l.line((a.1, a.0), (b.1, b.0), thin, dark)
                    }
                }
                let off = if r % 2 == 0 { 0.0 } else { bw / 2.0 };
                let mut x = -ax + off + bw;
                let y1 = y0 + ay * 2.0 / rows as f32;
                while x < ax - 0.05 {
                    let (a, b) = ((x, y0), (x, y1));
                    if long_x {
                        l.line(a, b, thin, dark)
                    } else {
                        l.line((a.1, a.0), (b.1, b.0), thin, dark)
                    }
                    x += bw;
                }
            }
        }
        _ => {}
    }

    // Cracks as it takes damage.
    let frac = e.hp_frac();
    if frac < 0.7 {
        let cracks = if frac < 0.35 { 3 } else { 1 };
        let mut s = e.seed;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s % 1000) as f32 / 1000.0
        };
        for _ in 0..cracks {
            let mut p = (hw * (rnd() * 1.6 - 0.8), hh * (rnd() * 1.6 - 0.8));
            for _ in 0..3 {
                let q = (
                    (p.0 + (rnd() - 0.5) * 0.6).clamp(-hw, hw),
                    (p.1 + (rnd() - 0.5) * 0.6).clamp(-hh, hh),
                );
                l.line(p, q, 0.05, Color::new(0.1, 0.05, 0.0, 0.8));
                p = q;
            }
        }
    }

    // Outline
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    for i in 0..4 {
        l.line(corners[i], corners[(i + 1) % 4], 0.06, dark);
    }
}

pub fn pig_colors(kind: PigKind) -> (Color, Color) {
    match kind {
        PigKind::Grunt => (rgb(247, 160, 180), rgb(220, 110, 140)),
        PigKind::Dasher => (rgb(255, 196, 90), rgb(220, 140, 50)),
        PigKind::Boomer => (rgb(95, 85, 110), rgb(60, 52, 72)),
        PigKind::Hog => (rgb(160, 105, 75), rgb(115, 72, 50)),
    }
}

/// A furious pig. Drawn in its own rotated frame.
pub fn pig(view: &View, kind: PigKind, pos: V2, angle: f32, time: f32) {
    let r = kind.radius();
    let l = Local::new(*view, pos, angle);
    let (body, dark) = pig_colors(kind);
    let outline = Color::new(0.0, 0.0, 0.0, 0.35);

    // Ears
    for sx in [-1.0, 1.0] {
        l.tri(
            (sx * 0.25 * r, 0.85 * r),
            (sx * 0.85 * r, 0.6 * r),
            (sx * 0.75 * r, 1.15 * r),
            dark,
        );
    }
    l.circle(0.0, 0.0, r * 1.06, outline);
    l.circle(0.0, 0.0, r, body);
    // Belly highlight
    l.circle(
        -0.3 * r,
        0.35 * r,
        0.25 * r,
        Color::new(1.0, 1.0, 1.0, 0.18),
    );

    // Eyes
    for sx in [-1.0, 1.0] {
        l.circle(sx * 0.33 * r, 0.22 * r, 0.2 * r, WHITE);
        l.circle(sx * 0.28 * r, 0.18 * r, 0.1 * r, BLACK);
    }
    // Furious brows, sloping down toward the middle.
    for sx in [-1.0, 1.0] {
        l.line(
            (sx * 0.6 * r, 0.55 * r),
            (sx * 0.1 * r, 0.35 * r),
            0.13 * r,
            rgb(60, 30, 30),
        );
    }
    // Snout
    l.ellipse(0.0, -0.22 * r, 0.38 * r, 0.26 * r, dark);
    l.circle(-0.13 * r, -0.22 * r, 0.07 * r, rgb(80, 30, 40));
    l.circle(0.13 * r, -0.22 * r, 0.07 * r, rgb(80, 30, 40));
    // Frown
    l.line(
        (-0.28 * r, -0.62 * r),
        (0.0, -0.52 * r),
        0.06 * r,
        rgb(80, 30, 40),
    );
    l.line(
        (0.0, -0.52 * r),
        (0.28 * r, -0.62 * r),
        0.06 * r,
        rgb(80, 30, 40),
    );

    match kind {
        PigKind::Hog => {
            for sx in [-1.0, 1.0] {
                l.tri(
                    (sx * 0.3 * r, -0.45 * r),
                    (sx * 0.42 * r, -0.4 * r),
                    (sx * 0.46 * r, -0.05 * r),
                    rgb(250, 245, 225),
                );
            }
        }
        PigKind::Boomer => {
            // Fuse on top with a flickering spark.
            l.line(
                (0.0, 0.95 * r),
                (0.2 * r, 1.35 * r),
                0.07 * r,
                rgb(60, 50, 40),
            );
            let flick = 0.12 + 0.05 * (time * 30.0).sin();
            l.circle(0.2 * r, 1.4 * r, flick * r * 1.6, rgb(255, 200, 60));
            l.circle(0.2 * r, 1.4 * r, flick * r * 0.8, rgb(255, 255, 220));
        }
        PigKind::Dasher => {
            // Headband with trailing tails
            l.line(
                (-0.92 * r, 0.42 * r),
                (0.92 * r, 0.42 * r),
                0.14 * r,
                rgb(220, 40, 40),
            );
            l.line(
                (-0.92 * r, 0.42 * r),
                (-1.35 * r, 0.25 * r),
                0.1 * r,
                rgb(220, 40, 40),
            );
            l.line(
                (-0.92 * r, 0.42 * r),
                (-1.3 * r, 0.55 * r),
                0.1 * r,
                rgb(220, 40, 40),
            );
        }
        PigKind::Grunt => {}
    }
}

/// A smug wolf. Gets visibly worried as it takes damage.
pub fn wolf(view: &View, e: &Ent) {
    let Shape::Ball { r } = e.shape else { return };
    let l = Local::new(*view, e.pos, e.angle);
    let fur = mix(rgb(140, 145, 160), rgb(230, 120, 120), e.hurt_flash * 0.6);
    let dark = rgb(90, 92, 108);
    let light = rgb(220, 222, 230);

    for sx in [-1.0, 1.0] {
        l.tri(
            (sx * 0.2 * r, 0.8 * r),
            (sx * 0.8 * r, 0.5 * r),
            (sx * 0.7 * r, 1.35 * r),
            dark,
        );
        l.tri(
            (sx * 0.35 * r, 0.8 * r),
            (sx * 0.7 * r, 0.62 * r),
            (sx * 0.65 * r, 1.12 * r),
            rgb(220, 160, 170),
        );
    }
    l.circle(0.0, 0.0, r * 1.05, Color::new(0.0, 0.0, 0.0, 0.35));
    l.circle(0.0, 0.0, r, fur);
    // Muzzle
    l.ellipse(0.0, -0.3 * r, 0.5 * r, 0.35 * r, light);
    l.circle(0.0, -0.12 * r, 0.13 * r, rgb(30, 30, 35));

    // Eyes: half-lidded and smug when healthy, wide when hurt.
    let worried = e.hp_frac() < 0.6;
    for sx in [-1.0, 1.0] {
        let (ex, ey) = (sx * 0.34 * r, 0.28 * r);
        l.circle(ex, ey, 0.17 * r, rgb(255, 240, 150));
        l.circle(ex + 0.03 * r, ey, 0.08 * r, BLACK);
        if !worried {
            // Heavy upper eyelid
            l.ellipse(ex, ey + 0.1 * r, 0.2 * r, 0.11 * r, fur);
            l.line(
                (ex - 0.2 * r, ey + 0.03 * r),
                (ex + 0.2 * r, ey + 0.03 * r),
                0.04 * r,
                dark,
            );
        }
    }
    if worried {
        // Sweat drop
        l.circle(0.75 * r, 0.45 * r, 0.1 * r, rgb(120, 190, 255));
        // Wobbly mouth
        l.line((-0.25 * r, -0.5 * r), (0.25 * r, -0.5 * r), 0.05 * r, dark);
    } else {
        // Smirk: lopsided line with a fang
        l.line((-0.3 * r, -0.45 * r), (0.1 * r, -0.5 * r), 0.05 * r, dark);
        l.line((0.1 * r, -0.5 * r), (0.35 * r, -0.36 * r), 0.05 * r, dark);
        l.tri(
            (0.12 * r, -0.5 * r),
            (0.22 * r, -0.47 * r),
            (0.17 * r, -0.62 * r),
            WHITE,
        );
    }
}

pub fn sling_back(view: &View, pig_pos: Option<V2>) {
    let wood = rgb(120, 80, 45);
    let (px, py) = sim::POUCH;
    let base = view.p(px, 0.0);
    let fork = view.p(px, 2.0);
    let la = view.p(px - 0.45, py + 0.15);
    let w = 0.32 * view.s;
    draw_line(base.x, base.y, fork.x, fork.y, w, wood);
    draw_line(fork.x, fork.y, la.x, la.y, w * 0.8, wood);
    if let Some(p) = pig_pos {
        let pp = view.v(p);
        draw_line(la.x, la.y, pp.x, pp.y, 0.14 * view.s, rgb(70, 40, 30));
    }
}

pub fn sling_front(view: &View, pig_pos: Option<V2>) {
    let wood = rgb(140, 95, 55);
    let (px, py) = sim::POUCH;
    let fork = view.p(px, 2.0);
    let ra = view.p(px + 0.45, py + 0.15);
    let w = 0.32 * view.s;
    draw_line(fork.x, fork.y, ra.x, ra.y, w * 0.8, wood);
    let la = view.p(px - 0.45, py + 0.15);
    let band = rgb(90, 50, 35);
    match pig_pos {
        Some(p) => {
            let pp = view.v(p);
            draw_line(ra.x, ra.y, pp.x, pp.y, 0.14 * view.s, band);
        }
        None => draw_line(la.x, la.y, ra.x, ra.y, 0.1 * view.s, band),
    }
}

pub fn trajectory(view: &View, pull: V2) {
    let pts = Game::trajectory(pull, 26, 0.07);
    for (i, p) in pts.iter().enumerate() {
        if p.y < 0.0 {
            break;
        }
        let a = 0.9 - i as f32 * 0.03;
        let q = view.v(*p);
        draw_circle(
            q.x,
            q.y,
            (0.13 - i as f32 * 0.003) * view.s,
            Color::new(1.0, 1.0, 1.0, a),
        );
    }
}

/// The pigs waiting their turn, lined up beside the sling.
pub fn queue(view: &View, game: &Game, time: f32) {
    let skip = if game.phase == Phase::Aiming { 1 } else { 0 };
    let mut x = sim::POUCH.0 - 1.4;
    for (i, &k) in game.pigs_left.iter().enumerate().skip(skip) {
        let r = k.radius();
        let hop = ((time * 3.0 + i as f32).sin().max(0.0)) * 0.15;
        pig(view, k, V2::new(x, r + hop), 0.0, time);
        x -= r * 2.0 + 0.3;
    }
}

pub fn star(c: Vec2, r: f32, filled: bool) {
    let col = if filled {
        rgb(255, 210, 60)
    } else {
        Color::new(0.0, 0.0, 0.0, 0.25)
    };
    let pts: Vec<Vec2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            c + vec2(a.cos(), a.sin()) * rr
        })
        .collect();
    for i in 0..10 {
        draw_triangle(c, pts[i], pts[(i + 1) % 10], col);
    }
}

pub fn text_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let dims = measure_text(text, None, size as u16, 1.0);
    let px = x - dims.width / 2.0;
    draw_text(
        text,
        px + size * 0.04,
        y + size * 0.04,
        size,
        Color::new(0.0, 0.0, 0.0, 0.35),
    );
    draw_text(text, px, y, size, color);
}
