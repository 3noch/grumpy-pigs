//! Holiday costumes (see `holiday`): Thanksgiving turkeys in place of
//! wolves and cherubs in place of fairies. The pigs' pilgrim hats live with
//! the rest of the pig art in `art`.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use crate::art::{Anim, Mood, gaze, pot, rig_for, twinkle};
use crate::gfx::{P, Rig, alpha, bez, blink, hash, mix, rgb, rgba};
use crate::render::View;
use crate::sim::{V2, WolfKind};

struct TurkeyLook {
    light: Color,
    base: Color,
    dark: Color,
    /// Tail feather bands from the outside edge in.
    bands: [Color; 3],
    feathers: usize,
    /// Tail length (in body radii).
    tail: f32,
}

fn turkey_look(kind: WolfKind, hurt: f32) -> TurkeyLook {
    let red = rgb(240, 110, 110);
    let mut look = match kind {
        // A fluffy young poult.
        WolfKind::Pup => TurkeyLook {
            light: rgb(246, 226, 180),
            base: rgb(214, 178, 118),
            dark: rgb(170, 130, 80),
            bands: [rgb(250, 236, 200), rgb(222, 180, 110), rgb(180, 134, 80)],
            feathers: 5,
            tail: 1.6,
        },
        // The Big Bad Gobbler: dark bronze with a huge fan.
        WolfKind::Boss => TurkeyLook {
            light: rgb(130, 110, 96),
            base: rgb(76, 58, 50),
            dark: rgb(50, 38, 34),
            bands: [rgb(244, 232, 200), rgb(176, 70, 40), rgb(70, 46, 34)],
            feathers: 11,
            tail: 2.0,
        },
        WolfKind::Wolf | WolfKind::Helmet => TurkeyLook {
            light: rgb(186, 128, 84),
            base: rgb(128, 80, 48),
            dark: rgb(92, 56, 34),
            bands: [rgb(244, 230, 196), rgb(214, 110, 44), rgb(110, 64, 36)],
            feathers: 9,
            tail: 1.85,
        },
    };
    look.light = mix(look.light, red, hurt * 0.4);
    look.base = mix(look.base, red, hurt * 0.5);
    look
}

/// A smug turkey (the wolves' Thanksgiving costume). Same arguments as
/// `art::wolf`.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn turkey(
    view: &View,
    kind: WolfKind,
    pos: V2,
    angle: f32,
    radius: f32,
    hp: f32,
    hurt: f32,
    a: &Anim,
) {
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
    let look = turkey_look(kind, hurt);
    let line = rgb(52, 32, 24);
    let ow = 0.07;

    // --- Tail fan: spreads wide when worried, rattles when laughing ---
    let spread = match a.mood {
        Mood::Worried | Mood::Hurt => 1.12,
        _ => 1.0,
    };
    let rattle = if a.mood == Mood::Laugh {
        (t * 30.0).sin() * 0.04
    } else {
        0.0
    };
    let n = look.feathers;
    for i in 0..n {
        // Badly beaten turkeys have lost a few feathers.
        if hp < 0.4 && i % 3 == 1 {
            continue;
        }
        let k = i as f32 / (n - 1) as f32;
        let th = PI / 2.0 + (k - 0.5) * 2.6 * spread + 0.04 * (t * 2.0 + i as f32).sin() + rattle;
        let len = look.tail * (0.92 + 0.08 * (1.0 - (k - 0.5).abs() * 2.0));
        let (c, s) = (th.cos(), th.sin());
        let feather = |frac: f32, grow: f32, col: Color| {
            let half = len * frac / 2.0;
            let mid = 0.15 + half;
            rig.ellipse(c * mid, -0.1 + s * mid, half + grow, 0.24 + grow, th, col);
        };
        feather(1.0, 0.04, line);
        feather(1.0, 0.0, look.bands[0]);
        feather(0.86, 0.0, look.bands[1]);
        feather(0.62, 0.0, look.bands[2]);
    }

    // --- Little orange feet ---
    let foot = rgb(232, 150, 50);
    for sx in [-1.0f32, 1.0] {
        let x = sx * 0.3;
        for toe in [-0.14f32, 0.0, 0.14] {
            rig.stroke(&[(x, -0.9), (x + toe, -1.02)], 0.07, foot);
        }
    }

    // --- Round body ---
    rig.radial((0.0, 0.0), |_| 1.0, ow, line, line, (0.0, 0.0));
    rig.radial(
        (0.0, 0.0),
        |_| 1.0,
        0.0,
        look.light,
        look.base,
        (-0.3, 0.45),
    );
    rig.ellipse(0.0, -0.5, 0.55, 0.42, 0.0, alpha(look.light, 0.55));
    rig.ellipse(0.0, -0.82, 0.55, 0.13, 0.0, alpha(look.dark, 0.4));
    rig.ellipse(-0.45, 0.52, 0.18, 0.1, 0.7, rgba(255, 255, 255, 0.35));
    // Wings folded at the sides, with a few feather lines.
    for sx in [-1.0f32, 1.0] {
        rig.blob(
            sx * 0.86,
            -0.32,
            0.2,
            0.4,
            sx * 0.35,
            look.base,
            look.dark,
            Some((line, 0.04)),
        );
        for j in 0..3 {
            let y = -0.05 - j as f32 * 0.17;
            rig.stroke(
                &[(sx * 0.78, y - 0.1), (sx * 0.94, y - 0.16)],
                0.03,
                alpha(line, 0.6),
            );
        }
    }
    if kind == WolfKind::Pup {
        // Downy tuft on top.
        for (i, x) in [-0.12f32, 0.0, 0.12].into_iter().enumerate() {
            let sway = (t * 3.0 + i as f32).sin() * 0.04;
            let pts = bez((x, 0.95), (x + 0.12 + sway, 1.12), (x + sway, 1.22), 6);
            rig.stroke(&pts, 0.08, line);
            rig.stroke(&pts, 0.04, look.light);
        }
    }

    // --- Eyes ---
    let gz = gaze(&rig, a.look, 0.06);
    let bl = blink(t, a.seed);
    for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (ex, ey) = (sx * 0.3, 0.3);
        match a.mood {
            Mood::Hurt => {
                let pts = [
                    (ex - sx * 0.12, ey + 0.09),
                    (ex + sx * 0.07, ey),
                    (ex - sx * 0.12, ey - 0.09),
                ];
                rig.stroke(&pts, 0.07, line);
                continue;
            }
            Mood::Laugh => {
                let arc = bez(
                    (ex - 0.13, ey - 0.03),
                    (ex, ey + 0.14),
                    (ex + 0.13, ey - 0.03),
                    8,
                );
                rig.stroke(&arc, 0.07, line);
                continue;
            }
            _ => {}
        }
        let worried = a.mood == Mood::Worried;
        let ry = if worried { 0.22 } else { 0.18 };
        rig.ellipse(ex, ey, 0.17 + ow * 0.7, ry + ow * 0.7, 0.0, line);
        rig.ellipse(ex, ey, 0.17, ry, 0.0, WHITE);
        let pr = if worried { 0.05 } else { 0.075 };
        let (px, py) = (ex + gz.0, ey + gz.1);
        rig.circle(px, py, pr, rgb(20, 16, 18));
        rig.circle(px - 0.025, py + 0.035, pr * 0.4, WHITE);
        if hp < 0.35 && i == 0 {
            let bruise: Vec<P> = (0..=20)
                .map(|k| {
                    let th = k as f32 / 20.0 * TAU;
                    (ex + th.cos() * 0.23, ey + th.sin() * 0.25)
                })
                .collect();
            rig.stroke(&bruise, 0.06, rgba(110, 60, 140, 0.7));
        }
        // Smug half-closed lids.
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
                        ex + th.cos() * (0.17 + ow * 0.7),
                        bottom + th.sin() * (top - bottom),
                    )
                })
                .collect();
            rig.poly(&lid_pts, look.base, None);
            rig.stroke(&[(ex - 0.19, bottom), (ex + 0.19, bottom)], 0.05, line);
        }
    }

    // --- Brows ---
    let brow_w = if kind == WolfKind::Boss { 0.12 } else { 0.07 };
    for sx in [-1.0f32, 1.0] {
        let (outer, inner) = match a.mood {
            Mood::Worried => (0.5, 0.6),
            Mood::Hurt => (0.56, 0.42),
            Mood::Laugh => (0.56, 0.54),
            _ if kind == WolfKind::Boss => (0.6, 0.48),
            _ if sx > 0.0 => (0.64 + 0.02 * (t * 1.5).sin(), 0.56),
            _ => (0.52, 0.52),
        };
        let pts = bez(
            (sx * 0.5, outer),
            (sx * 0.3, outer + 0.05),
            (sx * 0.12, inner),
            6,
        );
        rig.stroke(&pts, brow_w, line);
    }

    // --- Beak, snood and wattle ---
    let beak = rgb(248, 186, 62);
    let red = rgb(214, 50, 56);
    let open = match a.mood {
        Mood::Laugh => 0.1 + 0.05 * (t * 22.0).sin().abs(),
        Mood::Hurt | Mood::Worried => 0.08,
        _ => 0.0,
    };
    if kind != WolfKind::Pup {
        rig.ellipse(0.02, -0.36, 0.13, 0.17, 0.1, red);
        rig.ellipse(0.1, -0.44, 0.09, 0.12, 0.2, red);
    }
    if open > 0.0 {
        rig.poly(
            &[
                (-0.13, 0.02 - open),
                (0.13, 0.02 - open),
                (0.0, -0.2 - open),
            ],
            beak,
            Some((line, 0.035)),
        );
        rig.ellipse(
            0.0,
            0.03 - open * 0.5,
            0.11,
            open * 0.5,
            0.0,
            rgb(120, 30, 40),
        );
    }
    let tip = if kind == WolfKind::Pup { -0.08 } else { -0.16 };
    rig.poly(
        &[(-0.16, 0.1), (0.16, 0.1), (0.0, tip)],
        beak,
        Some((line, 0.035)),
    );
    if kind != WolfKind::Pup {
        let sway = 0.04 * (t * 2.5 + hash(a.seed, 4) * 6.0).sin();
        let len = if kind == WolfKind::Boss { 0.45 } else { 0.32 };
        let snood = bez(
            (0.02, 0.12),
            (0.2 + sway, 0.06),
            (0.16 + sway * 2.0, 0.12 - len),
            8,
        );
        rig.stroke(&snood, 0.13, line);
        rig.stroke(&snood, 0.08, red);
    }

    if kind == WolfKind::Helmet {
        pot(&rig, line);
    }
}

/// A chubby little cherub (the fairies' Thanksgiving costume), arms raised
/// to hold things up. Same arguments as `art::fairy`.
#[allow(clippy::too_many_lines)]
pub fn cherub(view: &View, pos: V2, r: f32, a: &Anim, startled: bool) {
    let t = a.t + hash(a.seed, 9) * 10.0;
    let rig = Rig::new(*view, pos, 0.0, r);
    let line = rgb(120, 70, 60);
    let skin = rgb(255, 214, 192);
    let skin_light = rgb(255, 236, 222);
    let curls = rgb(250, 204, 96);
    let gold = rgb(255, 216, 90);

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
        rgba(255, 246, 220, pulse),
        rgba(255, 246, 220, 0.0),
    );

    // Little feathered wings, beating.
    let speed = if startled { 30.0 } else { 14.0 };
    let beat = 0.35 * (t * speed).sin();
    for sx in [-1.0f32, 1.0] {
        for (j, (len, w)) in [(0.62f32, 0.2f32), (0.5, 0.17), (0.38, 0.14)]
            .into_iter()
            .enumerate()
        {
            let th = sx * (0.25 + j as f32 * 0.35) + sx * beat;
            let dir = V2::new(sx * th.cos().abs(), th.sin() * sx);
            let (cx, cy) = (sx * 0.22 + dir.x * len * 0.7, 0.05 + dir.y * len * 0.7);
            let rot = dir.y.atan2(dir.x);
            rig.ellipse(cx, cy, len * 0.6 + 0.03, w + 0.03, rot, rgb(190, 200, 214));
            rig.ellipse(cx, cy, len * 0.6, w, rot, rgb(255, 255, 255));
        }
    }

    // Chubby arms: up to hold things, flung wide when startled.
    for sx in [-1.0f32, 1.0] {
        let pts = if startled {
            bez((sx * 0.2, -0.15), (sx * 0.55, -0.05), (sx * 0.7, 0.3), 6)
        } else {
            bez((sx * 0.2, -0.15), (sx * 0.5, 0.35), (sx * 0.34, 0.86), 6)
        };
        rig.stroke(&pts, 0.22, line);
        rig.stroke(&pts, 0.16, skin);
        let (hx, hy) = pts[pts.len() - 1];
        rig.blob(hx, hy, 0.1, 0.1, 0.0, skin_light, skin, Some((line, 0.035)));
    }

    // Round tummy and pudgy legs.
    for sx in [-1.0f32, 1.0] {
        let kick = (t * 4.0 + sx).sin() * 0.04;
        rig.blob(
            sx * 0.17 + kick,
            -0.78,
            0.13,
            0.17,
            sx * 0.3,
            skin_light,
            skin,
            Some((line, 0.035)),
        );
    }
    rig.blob(
        0.0,
        -0.4,
        0.38,
        0.36,
        0.0,
        skin_light,
        skin,
        Some((line, 0.04)),
    );
    // A wisp of white cloth.
    rig.poly(
        &[(-0.37, -0.5), (0.37, -0.5), (0.25, -0.72), (-0.25, -0.72)],
        rgb(252, 250, 246),
        Some((rgb(200, 196, 190), 0.025)),
    );
    rig.stroke(&[(-0.3, -0.42), (0.3, -0.58)], 0.06, rgb(252, 250, 246));

    // Head with golden curls.
    for i in 0..9 {
        let th = 0.15 + i as f32 / 8.0 * (PI - 0.3);
        rig.blob(
            th.cos() * 0.3,
            0.3 + th.sin() * 0.27,
            0.11,
            0.11,
            0.0,
            rgb(255, 232, 150),
            curls,
            Some((line, 0.03)),
        );
    }
    rig.blob(
        0.0,
        0.28,
        0.3,
        0.28,
        0.0,
        skin_light,
        skin,
        Some((line, 0.04)),
    );
    // Face.
    let gz = gaze(&rig, a.look, 0.03);
    if startled {
        for sx in [-1.0f32, 1.0] {
            rig.circle(sx * 0.1, 0.3, 0.06, WHITE);
            rig.circle(sx * 0.1 + gz.0, 0.3 + gz.1, 0.032, rgb(60, 40, 40));
        }
        rig.ellipse(0.0, 0.15, 0.04, 0.05, 0.0, rgb(150, 60, 60));
    } else {
        let shut = blink(a.t, a.seed);
        for sx in [-1.0f32, 1.0] {
            rig.ellipse(
                sx * 0.1 + gz.0,
                0.3 + gz.1,
                0.035,
                0.045 * (1.0 - shut).max(0.15),
                0.0,
                rgb(60, 40, 40),
            );
        }
        let smile = bez((-0.07, 0.18), (0.0, 0.13), (0.07, 0.18), 5);
        rig.stroke(&smile, 0.03, rgb(150, 60, 60));
    }
    for sx in [-1.0f32, 1.0] {
        rig.circle(sx * 0.17, 0.21, 0.055, rgba(255, 120, 140, 0.45));
    }

    // Halo.
    let bob = 0.03 * (t * 2.0).sin();
    let halo: Vec<P> = (0..=24)
        .map(|i| {
            let th = i as f32 / 24.0 * TAU;
            (th.cos() * 0.26, 0.8 + bob + th.sin() * 0.07)
        })
        .collect();
    rig.stroke(&halo, 0.09, alpha(gold, 0.35));
    rig.stroke(&halo, 0.05, gold);

    // Twinkles.
    for i in 0..3 {
        let th = t * 1.4 + i as f32 * TAU / 3.0;
        let tw = 0.5 + 0.5 * (t * 6.0 + i as f32 * 2.0).sin();
        twinkle(
            &rig,
            th.cos() * 1.1,
            0.1 + th.sin() * 0.8,
            0.08 + 0.08 * tw,
            rgba(255, 246, 200, 0.5 + 0.5 * tw),
        );
    }
}
