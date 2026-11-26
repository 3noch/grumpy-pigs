//! Scene drawing: camera, scenery, blocks, sling, HUD helpers.
//! Characters live in `art`, primitives in `gfx`.

use std::f32::consts::FRAC_PI_2;

use macroquad::prelude::*;

use crate::art::{self, Anim, Mood};
use crate::gfx::{P, Rig, alpha, bez, hash, mix, rgb, rgba};
use crate::sim::{self, Ent, Game, Kind, Material, Phase, Shape, V2};

/// Maps world meters (y up) to screen pixels (y down), optionally mirrored
/// left-to-right so the sling sits on the right.
#[derive(Clone, Copy)]
pub struct View {
    pub s: f32,
    pub ox: f32,
    pub oy: f32,
    /// Screen width to mirror across, when mirrored.
    pub mirror: Option<f32>,
}

pub const VIEW_W: f32 = 42.0;
pub const VIEW_H: f32 = 22.0;
pub const VIEW_X0: f32 = -1.0;
pub const VIEW_Y0: f32 = -1.5;

impl View {
    pub fn fit(shake: Vec2, mirrored: bool) -> Self {
        let area = Rect::new(shake.x, shake.y, screen_width(), screen_height());
        Self::fit_in(area, mirrored)
    }

    /// Fits the play area inside `area`, anchored to its bottom edge.
    pub fn fit_in(area: Rect, mirrored: bool) -> Self {
        let s = (area.w / VIEW_W).min(area.h / VIEW_H);
        let ox = area.x + (area.w - VIEW_W * s) / 2.0 - VIEW_X0 * s;
        let oy = area.bottom() + VIEW_Y0 * s;
        View {
            s,
            ox,
            oy,
            mirror: mirrored.then_some(screen_width()),
        }
    }
    pub fn p(&self, x: f32, y: f32) -> Vec2 {
        let sx = self.ox + x * self.s;
        let sx = self.mirror.map_or(sx, |w| w - sx);
        vec2(sx, self.oy - y * self.s)
    }
    /// Screen-space rotation for something rotated `angle` in the world.
    pub fn rotation(&self, angle: f32) -> f32 {
        if self.mirror.is_some() { angle } else { -angle }
    }
    pub fn v(&self, v: V2) -> Vec2 {
        self.p(v.x, v.y)
    }
    /// Screen point → world point (the inverse of [`View::p`]).
    pub fn unproject(&self, p: Vec2) -> V2 {
        let sx = self.mirror.map_or(p.x, |w| w - p.x);
        V2::new((sx - self.ox) / self.s, (self.oy - p.y) / self.s)
    }
    /// Leftmost / rightmost visible world x.
    pub fn x_range(&self) -> (f32, f32) {
        (-self.ox / self.s, (screen_width() - self.ox) / self.s)
    }
}

pub fn background(view: &View, time: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let top = rgb(98, 170, 232);
    let bottom = rgb(214, 238, 250);
    let strips = 32;
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
    let sun = view.p(36.0, 17.0);
    for (r, a) in [(3.2, 0.12), (2.4, 0.2), (1.9, 0.35)] {
        draw_circle(sun.x, sun.y, r * view.s, rgba(255, 245, 200, a));
    }
    draw_circle(sun.x, sun.y, 1.5 * view.s, rgb(255, 242, 180));

    // Clouds with a soft underside.
    for (i, (y, sz, speed)) in [(15.0, 1.0, 0.3), (12.5, 0.7, 0.5), (17.5, 1.3, 0.2)]
        .iter()
        .enumerate()
    {
        let span = VIEW_W + 12.0;
        let x = ((i as f32 * 17.0 + time * speed) % span) - 6.0;
        let puffs = [
            (0.0, 0.0, 1.0),
            (1.1, 0.35, 1.25),
            (2.3, 0.0, 0.95),
            (1.2, -0.25, 1.0),
        ];
        for (dx, dy, r) in puffs {
            let p = view.p(x + dx * sz, y + (dy - 0.12) * sz);
            draw_circle(p.x, p.y, r * sz * view.s, rgba(190, 210, 235, 0.9));
        }
        for (dx, dy, r) in puffs {
            let p = view.p(x + dx * sz, y + dy * sz);
            draw_circle(p.x, p.y, r * sz * view.s * 0.94, rgba(255, 255, 255, 0.97));
        }
    }

    // Rolling hills, far then near, with a few round trees.
    let far = rgb(150, 205, 140);
    let near = rgb(116, 184, 104);
    for &(x, r) in &[(-2.0, 9.0), (28.0, 8.0)] {
        let p = view.p(x, -r * 0.55);
        draw_circle(p.x, p.y, r * view.s, far);
    }
    for &(x, h, s) in &[(1.0, 3.2, 1.0), (27.5, 2.8, 0.8), (31.0, 3.0, 0.9)] {
        tree(view, x, h, s, time);
    }
    for &(x, r) in &[(12.0, 11.0), (42.0, 12.0)] {
        let p = view.p(x, -r * 0.55);
        draw_circle(p.x, p.y, r * view.s, near);
    }

    // Ground: grass lip, soil with pebbles.
    let g = view.p(0.0, 0.0);
    draw_rectangle(0.0, g.y, sw, sh - g.y + 1.0, rgb(126, 88, 58));
    let (x0, x1) = view.x_range();
    let mut x = x0.floor();
    while x < x1 {
        let k = hash(x.to_bits(), 5);
        let p = view.p(x + k, -0.6 - k * 0.7);
        draw_ellipse(
            p.x,
            p.y,
            (0.12 + k * 0.1) * view.s,
            0.07 * view.s,
            0.0,
            rgb(104, 72, 48),
        );
        x += 1.3;
    }
    draw_rectangle(0.0, g.y, sw, 0.38 * view.s, rgb(98, 172, 74));
    draw_rectangle(0.0, g.y + 0.38 * view.s, sw, 0.1 * view.s, rgb(74, 136, 56));
}

fn tree(view: &View, x: f32, h: f32, s: f32, time: f32) {
    let sway = (time * 0.8 + x).sin() * 0.06;
    let base = view.p(x, 0.0);
    let top = view.p(x + sway, h * s);
    draw_line(
        base.x,
        base.y,
        top.x,
        top.y,
        0.35 * s * view.s,
        rgb(110, 82, 60),
    );
    let leaf_dark = rgb(70, 140, 70);
    let leaf = rgb(92, 168, 84);
    for (dx, dy, r) in [(-0.6, 0.0, 0.9), (0.6, 0.1, 0.85), (0.0, 0.7, 1.0)] {
        let p = view.p(x + sway + dx * s, h * s + dy * s);
        draw_circle(p.x, p.y, r * s * view.s, leaf_dark);
    }
    for (dx, dy, r) in [(-0.6, 0.05, 0.75), (0.6, 0.15, 0.7), (0.0, 0.78, 0.85)] {
        let p = view.p(x + sway + dx * s - 0.1, h * s + dy * s + 0.1);
        draw_circle(p.x, p.y, r * s * view.s, leaf);
    }
}

/// Swaying grass tufts drawn in front of the scene.
pub fn grass(view: &View, time: f32) {
    let (x0, x1) = view.x_range();
    let mut x = x0.floor();
    let col = rgb(84, 158, 64);
    let tip = rgb(130, 200, 96);
    while x < x1 {
        let k = hash(x.to_bits(), 3);
        if k > 0.35 {
            let bx = x + k * 0.6;
            for b in 0..3 {
                let off = (b as f32 - 1.0) * 0.12;
                let h = 0.35 + 0.2 * hash(x.to_bits(), b + 10);
                let sway = (time * 1.6 + bx * 0.7).sin() * 0.1 + off * 0.8;
                let a = view.p(bx + off - 0.05, 0.25);
                let c = view.p(bx + off + 0.05, 0.25);
                let t = view.p(bx + off + sway, 0.25 + h);
                draw_triangle(a, c, t, mix(col, tip, hash(x.to_bits(), b)));
            }
        }
        x += 0.9;
    }
}

fn material_colors(m: Material) -> (Color, Color, Color) {
    // (light, base, line)
    match m {
        Material::Straw => (rgb(250, 226, 140), rgb(228, 188, 92), rgb(150, 110, 40)),
        Material::Wood => (rgb(208, 150, 96), rgb(170, 110, 62), rgb(92, 56, 30)),
        Material::Brick => (rgb(206, 108, 86), rgb(168, 74, 58), rgb(92, 40, 32)),
        Material::Ice => (rgb(232, 248, 255), rgb(168, 214, 240), rgb(84, 140, 182)),
        Material::Tnt => (rgb(236, 92, 70), rgb(186, 44, 34), rgb(88, 20, 16)),
        Material::Stone => (rgb(186, 180, 170), rgb(130, 124, 118), rgb(66, 62, 58)),
    }
}

/// A rig whose local x axis runs along the block's long side, in world units.
fn block_rig(view: &View, e: &Ent, hw: f32, hh: f32) -> (Rig, f32, f32) {
    if hw >= hh {
        (Rig::new(*view, e.pos, e.angle, 1.0), hw, hh)
    } else {
        (Rig::new(*view, e.pos, e.angle + FRAC_PI_2, 1.0), hh, hw)
    }
}

fn rect_pts(x0: f32, y0: f32, x1: f32, y1: f32) -> [P; 4] {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

#[allow(clippy::too_many_lines)]
pub fn block(view: &View, e: &Ent) {
    let (hw, hh) = match e.shape {
        Shape::Box { hw, hh } => (hw, hh),
        Shape::Ball { r } => return boulder(view, e, r),
    };
    if e.kind == Kind::Rock {
        rock(view, e, hw, hh);
        return;
    }
    let Kind::Block(mat) = e.kind else { return };
    let (rig, l, w) = block_rig(view, e, hw, hh);
    let (light, base, line) = material_colors(mat);
    let flash = e.hurt_flash * 0.4;
    let light = mix(light, WHITE, flash);
    let base = mix(base, WHITE, flash);
    let seed = e.seed;

    match mat {
        Material::Straw => {
            // Frayed ends poke out past the bundle.
            for end in [-1.0f32, 1.0] {
                for i in 0..6 {
                    let y = -w + (i as f32 + 0.5) / 6.0 * 2.0 * w;
                    let len = 0.06 + 0.08 * hash(seed, i + if end > 0.0 { 20 } else { 40 });
                    rig.stroke(
                        &[(end * (l - 0.02), y), (end * (l + len), y + 0.02)],
                        0.03,
                        line,
                    );
                }
            }
            rig.poly(&rect_pts(-l, -w, l, w), base, Some((line, 0.03)));
            rig.rect_grad(-l, -w, l, w, light, base);
            let n = crate::gfx::count((w * 2.0) / 0.07, 64).max(2);
            for i in 1..n {
                let y = -w + i as f32 / n as f32 * 2.0 * w;
                let shade = mix(line, base, 0.35 + 0.4 * hash(seed, i));
                rig.stroke(&[(-l, y), (l, y)], 0.018, alpha(shade, 0.8));
            }
            // Twine ties
            for f in [-0.55, 0.55] {
                let x = f * l;
                rig.poly(
                    &rect_pts(x - 0.06, -w, x + 0.06, w),
                    rgb(170, 120, 60),
                    Some((line, 0.015)),
                );
            }
        }
        Material::Wood => {
            rig.poly(&rect_pts(-l, -w, l, w), base, Some((line, 0.035)));
            rig.rect_grad(-l, -w, l, w, light, base);
            // Wavy grain
            for (i, f) in (0u32..).zip([-0.5f32, 0.05, 0.55]) {
                let ph = hash(seed, i) * 6.0;
                let pts: Vec<P> = (0..=16)
                    .map(|k| {
                        let x = -l * 0.92 + k as f32 / 16.0 * l * 1.84;
                        (x, f * w + 0.04 * w * (x * 3.0 + ph).sin())
                    })
                    .collect();
                rig.stroke(&pts, 0.022, alpha(line, 0.55));
            }
            // Knot
            let kx = (hash(seed, 7) - 0.5) * l;
            rig.ellipse(kx, 0.0, 0.11, w * 0.32, 0.0, alpha(line, 0.5));
            rig.ellipse(kx, 0.0, 0.05, w * 0.14, 0.0, alpha(line, 0.7));
            // Nails
            for end in [-1.0, 1.0] {
                rig.circle(end * (l - 0.12), 0.0, 0.035, rgb(120, 120, 130));
            }
            rig.stroke(
                &[(-l + 0.03, w - 0.03), (l - 0.03, w - 0.03)],
                0.025,
                alpha(WHITE, 0.25),
            );
        }
        Material::Brick => {
            rig.poly(
                &rect_pts(-l, -w, l, w),
                rgb(196, 176, 150),
                Some((line, 0.035)),
            );
            let rows = crate::gfx::count(((w * 2.0) / 0.25).round(), 64).max(1);
            let rh = w * 2.0 / rows as f32;
            let bw = 0.5;
            for r in 0..rows {
                let y0 = -w + r as f32 * rh;
                let off = if r % 2 == 0 { 0.0 } else { -bw / 2.0 };
                let mut x = -l + off;
                let mut k = 0;
                while x < l {
                    let (a, b) = (x.max(-l) + 0.025, (x + bw).min(l) - 0.025);
                    if b > a {
                        let v = hash(seed, r * 31 + k);
                        let c = mix(base, light, v * 0.5);
                        rig.rect_grad(a, y0 + 0.025, b, y0 + rh - 0.025, mix(c, WHITE, 0.08), c);
                    }
                    x += bw;
                    k += 1;
                }
            }
        }
        Material::Ice => {
            rig.poly(
                &rect_pts(-l, -w, l, w),
                alpha(base, 0.8),
                Some((line, 0.035)),
            );
            rig.rect_grad(-l, -w, l, w, alpha(light, 0.85), alpha(base, 0.6));
            // Glints
            for (i, f) in (0u32..).zip([-0.55f32, 0.1, 0.6]) {
                let x = f * l;
                let k = 0.5 + hash(seed, i) * 0.5;
                rig.stroke(
                    &[(x - w * 0.6 * k, -w * 0.6), (x + w * 0.6 * k, w * 0.6)],
                    0.05,
                    rgba(255, 255, 255, 0.7),
                );
            }
            rig.stroke(
                &[(-l + 0.05, w - 0.05), (l - 0.05, w - 0.05)],
                0.04,
                rgba(255, 255, 255, 0.8),
            );
        }
        Material::Tnt | Material::Stone => {
            rig.poly(&rect_pts(-l, -w, l, w), base, Some((line, 0.04)));
            rig.rect_grad(-l, -w, l, w, light, base);
            // Crate slats
            for x in [-l * 0.5, 0.0, l * 0.5] {
                rig.stroke(&[(x, -w + 0.06), (x, w - 0.06)], 0.025, alpha(line, 0.4));
            }
            let inset = 0.06;
            let frame = [
                (-l + inset, -w + inset),
                (l - inset, -w + inset),
                (l - inset, w - inset),
                (-l + inset, w - inset),
                (-l + inset, -w + inset),
            ];
            rig.stroke(&frame, 0.05, line);
            if mat == Material::Tnt {
                rig.poly(
                    &rect_pts(-l, -w * 0.42, l, w * 0.42),
                    rgb(250, 238, 210),
                    Some((line, 0.02)),
                );
                let c = view.v(e.pos);
                let size = font_size(w * 1.05 * view.s);
                let dims = measure_text("TNT", None, size, 1.0);
                let rot = view.rotation(e.angle);
                let (s, co) = rot.sin_cos();
                let (ox, oy) = (-dims.width / 2.0, dims.offset_y / 2.0);
                draw_text_ex(
                    "TNT",
                    c.x + ox * co - oy * s,
                    c.y + ox * s + oy * co,
                    TextParams {
                        font_size: size,
                        rotation: rot,
                        color: line,
                        ..Default::default()
                    },
                );
            }
        }
    }

    // Cracks as it takes damage.
    let frac = e.hp_frac();
    if frac < 0.7 {
        let cracks = if frac < 0.35 { 3 } else { 1 };
        for c in 0..cracks {
            let mut p = (
                l * (hash(seed, 100 + c) * 1.6 - 0.8),
                w * (hash(seed, 110 + c) * 1.6 - 0.8),
            );
            let mut pts = vec![p];
            for s in 0..4 {
                p = (
                    (p.0 + (hash(seed, 120 + c * 7 + s) - 0.5) * 0.5).clamp(-l, l),
                    (p.1 + (hash(seed, 140 + c * 7 + s) - 0.5) * 0.5).clamp(-w, w),
                );
                pts.push(p);
            }
            rig.stroke(&pts, 0.045, rgba(40, 20, 10, 0.75));
        }
    }
}

/// A round stone boulder, rolling with its body.
fn boulder(view: &View, e: &Ent, r: f32) {
    let rig = Rig::new(*view, e.pos, e.angle, r);
    let (light, base, line) = material_colors(Material::Stone);
    let seed = e.seed;
    let lumpy = move |th: f32| {
        1.0 + 0.05 * (th * 5.0 + hash(seed, 1) * 6.0).sin() + 0.03 * (th * 9.0).sin()
    };
    rig.radial((0.0, 0.0), lumpy, 0.06, line, line, (0.0, 0.0));
    rig.radial((0.0, 0.0), lumpy, 0.0, light, base, (-0.3, 0.35));
    for i in 0..7 {
        let a = hash(seed, i + 10) * std::f32::consts::TAU;
        let d = 0.25 + hash(seed, i + 20) * 0.55;
        let rr = 0.05 + hash(seed, i + 30) * 0.08;
        rig.circle(a.cos() * d, a.sin() * d, rr, alpha(line, 0.35));
    }
    rig.stroke(
        &bez((-0.5, -0.2), (-0.1, 0.0), (0.2, -0.45), 6),
        0.04,
        alpha(line, 0.5),
    );
    rig.ellipse(-0.35, 0.45, 0.22, 0.1, 0.6, rgba(255, 255, 255, 0.3));
}

fn rock(view: &View, e: &Ent, hw: f32, hh: f32) {
    let rig = Rig::new(*view, e.pos, e.angle, 1.0);
    let line = rgb(70, 50, 36);
    rig.poly(
        &rect_pts(-hw, -hh, hw, hh),
        rgb(140, 102, 72),
        Some((line, 0.05)),
    );
    rig.rect_grad(-hw, -hh, hw, hh, rgb(150, 112, 80), rgb(112, 78, 54));
    for i in 0..14 {
        let x = (hash(e.seed, i) * 2.0 - 1.0) * (hw - 0.4);
        let y = (hash(e.seed, i + 50) * 2.0 - 1.0) * (hh - 0.4);
        let r = 0.15 + hash(e.seed, i + 99) * 0.25;
        rig.blob(
            x,
            y,
            r,
            r * 0.7,
            0.0,
            rgb(176, 170, 160),
            rgb(120, 114, 108),
            Some((line, 0.03)),
        );
    }
    rig.rect_grad(-hw, hh - 0.35, hw, hh, rgb(118, 190, 88), rgb(84, 156, 62));
}

/// The slingshot's back half (behind the pig). `base` is where it stands;
/// everything below is drawn relative to it.
pub fn sling_back(view: &View, base: V2, pig_pos: Option<V2>) {
    let rig = Rig::new(*view, V2::new(0.0, base.y), 0.0, 1.0);
    let (px, py) = (base.x, sim::SLING_HEIGHT);
    let pig_pos = pig_pos.map(|p| V2::new(p.x, p.y - base.y));
    let bark = rgb(120, 80, 45);
    let line = rgb(60, 36, 20);
    // Trunk: tapered with a highlight.
    let trunk = [
        (px - 0.2, 0.0),
        (px + 0.2, 0.0),
        (px + 0.15, 2.0),
        (px - 0.15, 2.0),
    ];
    rig.poly(&trunk, bark, Some((line, 0.05)));
    rig.stroke(
        &[(px - 0.06, 0.1), (px - 0.05, 1.9)],
        0.06,
        rgba(255, 220, 180, 0.25),
    );
    let arm = bez((px, 1.9), (px - 0.25, 2.4), (px - 0.45, py + 0.2), 8);
    rig.stroke(&arm, 0.3, line);
    rig.stroke(&arm, 0.22, bark);
    let (la, ra) = ((px - 0.45, py + 0.15), (px + 0.45, py + 0.15));
    if let Some(p) = pig_pos {
        // Both bands run to a leather pouch cupping the back of the pig.
        let pouch = (p.x - 0.12, p.y);
        rig.stroke(&[la, pouch], 0.16, rgb(70, 30, 30));
        rig.stroke(&[ra, pouch], 0.16, rgb(92, 40, 36));
        rig.blob(
            pouch.0,
            pouch.1,
            0.3,
            0.42,
            0.0,
            rgb(140, 90, 60),
            rgb(96, 58, 36),
            Some((line, 0.04)),
        );
    }
}

pub fn sling_front(view: &View, base: V2, pig_pos: Option<V2>) {
    let rig = Rig::new(*view, V2::new(0.0, base.y), 0.0, 1.0);
    let (px, py) = (base.x, sim::SLING_HEIGHT);
    let bark = rgb(140, 95, 55);
    let line = rgb(60, 36, 20);
    let arm = bez((px, 1.9), (px + 0.25, 2.4), (px + 0.45, py + 0.2), 8);
    rig.stroke(&arm, 0.3, line);
    rig.stroke(&arm, 0.22, bark);
    let (la, ra) = ((px - 0.45, py + 0.15), (px + 0.45, py + 0.15));
    if pig_pos.is_none() {
        let sag = bez(la, (px, py - 0.25), ra, 8);
        rig.stroke(&sag, 0.12, rgb(92, 40, 36));
    }
    for tip in [la, ra] {
        rig.circle(tip.0, tip.1 + 0.05, 0.14, line);
        rig.circle(tip.0, tip.1 + 0.05, 0.1, bark);
    }
}

pub fn trajectory(view: &View, game: &Game, pull: V2, time: f32) {
    let pts = game.trajectory(pull, 26, 0.07);
    let floor = game.launcher.y.min(0.0) - 1.0;
    let march = (time * 2.0) % 1.0;
    for (i, p) in pts.iter().enumerate() {
        if p.y < floor {
            break;
        }
        let fi = i as f32 + march;
        let a = (0.9 - fi * 0.03).max(0.0);
        let q = view.v(*p);
        draw_circle(
            q.x,
            q.y,
            (0.14 - fi * 0.003) * view.s,
            rgba(80, 60, 60, a * 0.35),
        );
        draw_circle(
            q.x,
            q.y,
            (0.11 - fi * 0.003) * view.s,
            rgba(255, 255, 255, a),
        );
    }
}

/// The pigs waiting their turn, lined up beside the sling.
pub fn queue(view: &View, game: &Game, time: f32) {
    let won = matches!(game.phase, Phase::Won { .. });
    let skip = usize::from(game.phase == Phase::Aiming);
    let mut x = game.launcher.x - 1.4;
    let ground = game.launcher.y;
    for (i, &k) in game.pigs_left.iter().enumerate().skip(skip) {
        let r = k.radius();
        let hop_speed = if won { 9.0 } else { 3.0 };
        let height = if won { 0.6 } else { 0.15 };
        let hop = ((time * hop_speed + i as f32 * 1.3).sin().max(0.0)) * height;
        let seed = u32::try_from(i).unwrap_or(0).wrapping_mul(7919) + 17;
        let mood = if won { Mood::Cheer } else { Mood::Idle };
        let mut anim = Anim::new(time, seed, mood, V2::new(1.0, 0.15));
        if hop < 0.01 && !won {
            // Squash a touch on landing.
            anim.stretch = -0.06 * ((time * hop_speed + i as f32 * 1.3).sin().abs());
        }
        art::pig(view, k, V2::new(x, ground + r + hop), 0.0, &anim);
        x -= r * 2.0 + 0.35;
    }
}

pub fn star(c: Vec2, r: f32, filled: bool) {
    let col = if filled {
        rgb(255, 210, 60)
    } else {
        rgba(0, 0, 0, 0.25)
    };
    let pts: Vec<Vec2> = (0..10)
        .map(|i| {
            let a = -FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            c + vec2(a.cos(), a.sin()) * rr
        })
        .collect();
    if filled {
        for i in 0..10 {
            draw_triangle(
                c + vec2(0.0, r * 0.08),
                pts[i] + vec2(0.0, r * 0.08),
                pts[(i + 1) % 10] + vec2(0.0, r * 0.08),
                rgb(200, 140, 20),
            );
        }
    }
    for i in 0..10 {
        draw_triangle(c, pts[i], pts[(i + 1) % 10], col);
    }
}

/// Pixel size → macroquad font size.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn font_size(px: f32) -> u16 {
    px.clamp(1.0, 1000.0) as u16
}

pub fn text_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let dims = measure_text(text, None, font_size(size), 1.0);
    let px = x - dims.width / 2.0;
    draw_text(
        text,
        px + size * 0.04,
        y + size * 0.04,
        size,
        Color::new(0.0, 0.0, 0.0, 0.35 * color.a),
    );
    draw_text(text, px, y, size, color);
}
