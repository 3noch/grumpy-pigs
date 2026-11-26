//! Level definitions. Ground top is y = 0; the sling sits at x ≈ 6.

use crate::sim::{
    Launcher, Level, Material,
    Material::{Brick, Ice, Straw, Tnt, Wood},
    Piece, PieceKind,
    PigKind::{Boomer, Dasher, Grunt, Hog, Litter, Puffer, Thumper},
    Size, Theme, WolfKind,
};

const T: f32 = 0.4; // thickness of posts and planks

#[derive(Default)]
struct B {
    pieces: Vec<Piece>,
}

impl B {
    fn add(&mut self, kind: PieceKind, x: f32, y: f32, angle: f32, size: Size) {
        self.pieces.push(Piece {
            kind,
            x,
            y,
            angle,
            size,
        });
    }
    fn rect(&mut self, cx: f32, bottom: f32, w: f32, h: f32, mat: Material) -> f32 {
        let size = Size::Box { w, h };
        self.add(PieceKind::Block(mat), cx, bottom + h / 2.0, 0.0, size);
        bottom + h
    }
    /// Vertical post with its bottom at `y`. Returns its top.
    fn post(&mut self, x: f32, y: f32, h: f32, mat: Material) -> f32 {
        self.rect(x, y, T, h, mat)
    }
    /// Horizontal plank. Returns its top.
    fn plank(&mut self, cx: f32, y: f32, w: f32, mat: Material) -> f32 {
        self.rect(cx, y, w, T, mat)
    }
    /// Two posts and a plank roof. Returns the roof top.
    fn frame(&mut self, cx: f32, y: f32, span: f32, h: f32, mat: Material) -> f32 {
        let inset = span / 2.0 - T / 2.0;
        self.post(cx - inset, y, h, mat);
        self.post(cx + inset, y, h, mat);
        self.plank(cx, y + h, span, mat)
    }
    fn cube(&mut self, cx: f32, y: f32, s: f32, mat: Material) -> f32 {
        self.rect(cx, y, s, s, mat)
    }
    /// An explosive crate with its bottom at `y`.
    fn tnt(&mut self, cx: f32, y: f32) -> f32 {
        self.cube(cx, y, 0.8, Tnt)
    }
    fn critter(&mut self, x: f32, y: f32, r: f32, kind: WolfKind) {
        self.add(PieceKind::Wolf(kind), x, y + r, 0.0, Size::Ball { r });
    }
    /// A regular wolf of radius `r`, standing at `y`.
    fn wolf(&mut self, x: f32, y: f32, r: f32) {
        self.critter(x, y, r, WolfKind::Wolf);
    }
    fn pup(&mut self, x: f32, y: f32) {
        self.critter(x, y, WolfKind::Pup.radius(), WolfKind::Pup);
    }
    fn helmet(&mut self, x: f32, y: f32) {
        self.critter(x, y, WolfKind::Helmet.radius(), WolfKind::Helmet);
    }
    fn boss(&mut self, x: f32, y: f32) {
        self.critter(x, y, WolfKind::Boss.radius(), WolfKind::Boss);
    }
    /// A round stone resting at `y`.
    fn boulder(&mut self, x: f32, y: f32, r: f32) {
        self.add(PieceKind::Boulder, x, y + r, 0.0, Size::Ball { r });
    }
    /// Solid terrain from the ground up to `top`. Returns `top`.
    fn hill(&mut self, cx: f32, top: f32, w: f32) -> f32 {
        let size = Size::Box { w, h: top };
        self.add(PieceKind::Terrain, cx, top / 2.0, 0.0, size);
        top
    }
    /// A solid slope whose top surface runs from (x0, y0) to (x1, y1).
    fn ramp(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let thick = 0.6;
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        let angle = dy.atan2(dx);
        // Push the box below the surface line along its downward normal.
        let (nx, ny) = (angle.sin(), -angle.cos());
        let size = Size::Box { w: len, h: thick };
        self.add(
            PieceKind::Terrain,
            f32::midpoint(x0, x1) + nx * thick / 2.0,
            f32::midpoint(y0, y1) + ny * thick / 2.0,
            angle,
            size,
        );
    }
    fn build(self, name: &'static str, pigs: Vec<crate::sim::PigKind>) -> Level {
        Level {
            name: name.to_owned(),
            author: "Grumpy Pigs".to_owned(),
            based_on: String::new(),
            theme: Theme::Meadow,
            launcher: Launcher::default(),
            pieces: self.pieces,
            pigs,
        }
    }
}

pub fn all() -> Vec<Level> {
    vec![
        straw_hut(),
        stick_stack(),
        brick_layer(),
        hilltop(),
        big_bad_tower(),
        piggy_pile(),
        thin_ice(),
        powder_keg(),
        rock_and_roll(),
        tin_hats(),
        twin_peaks(),
        big_bad_boss(),
    ]
}

fn straw_hut() -> Level {
    let mut b = B::default();
    let top = b.frame(27.0, 0.0, 3.0, 2.0, Straw);
    b.wolf(27.0, 0.0, 0.55);
    b.wolf(27.0, top, 0.55);
    b.frame(32.0, 0.0, 2.4, 1.6, Straw);
    b.wolf(32.0, 0.0, 0.5);
    b.build("Straw Hut", vec![Grunt, Grunt, Grunt])
}

fn stick_stack() -> Level {
    let mut b = B::default();
    let t = b.frame(25.0, 0.0, 3.2, 2.2, Wood);
    b.wolf(25.0, 0.0, 0.55);
    b.frame(25.0, t, 3.2, 2.2, Wood);
    b.wolf(25.0, t, 0.55);

    let t = b.frame(31.0, 0.0, 3.2, 2.2, Wood);
    b.wolf(31.0, 0.0, 0.55);
    let t = b.cube(30.0, t, 0.8, Straw);
    b.cube(32.0, t - 0.8, 0.8, Straw);
    b.wolf(31.0, t - 0.8, 0.5);
    b.build("Stick Stack", vec![Grunt, Dasher, Grunt])
}

fn brick_layer() -> Level {
    let mut b = B::default();
    let t = b.frame(28.0, 0.0, 3.6, 2.0, Brick);
    b.wolf(28.0, 0.0, 0.55);
    let t = b.frame(28.0, t, 3.0, 2.0, Wood);
    b.wolf(28.0, t - 2.4, 0.5);
    b.wolf(28.0, t, 0.5);

    // A flimsy straw lookout to the side.
    let t = b.post(33.0, 0.0, 3.0, Straw);
    let t2 = b.post(34.6, 0.0, 3.0, Straw);
    let top = b.plank(33.8, t.max(t2), 2.0, Straw);
    b.wolf(33.8, top, 0.5);
    b.build("Brick Layer", vec![Hog, Dasher, Grunt])
}

fn hilltop() -> Level {
    let mut b = B::default();
    let h = b.hill(30.0, 3.0, 8.4);
    let t1 = b.frame(28.0, h, 2.8, 2.0, Wood);
    b.wolf(28.0, h, 0.5);
    b.frame(32.0, h, 2.8, 2.0, Brick);
    b.wolf(32.0, h, 0.5);
    let roof = b.plank(30.0, t1, 6.8, Wood);
    b.wolf(30.0, roof, 0.55);

    // Hiding behind a straw wall at the bottom of the hill.
    b.post(35.6, 0.0, 1.8, Straw);
    b.post(36.0, 0.0, 1.8, Straw);
    b.wolf(37.2, 0.0, 0.5);
    b.build("Hilltop", vec![Dasher, Boomer, Hog, Grunt])
}

fn big_bad_tower() -> Level {
    let mut b = B::default();
    let t = b.frame(31.0, 0.0, 4.2, 2.4, Brick);
    b.wolf(30.1, 0.0, 0.5);
    b.wolf(31.9, 0.0, 0.5);
    let t2 = b.frame(31.0, t, 3.4, 2.2, Wood);
    b.wolf(31.0, t, 0.55);
    let t3 = b.frame(31.0, t2, 2.6, 2.0, Wood);
    b.wolf(31.0, t2, 0.5);
    let t4 = b.frame(31.0, t3, 1.8, 1.4, Straw);
    b.wolf(31.0, t4, 0.6); // the Big Bad Wolf himself

    // Brick barricade out front.
    let w = b.post(25.0, 0.0, 2.0, Brick);
    b.cube(25.0, w, 0.8, Wood);
    b.post(25.5, 0.0, 2.0, Brick);
    b.build("Big Bad Tower", vec![Boomer, Dasher, Hog, Boomer])
}

/// Introduces the Litter (split) and pups.
fn piggy_pile() -> Level {
    let mut b = B::default();
    b.frame(22.0, 0.0, 2.0, 1.4, Straw);
    b.pup(22.0, 0.0);
    let t = b.frame(26.0, 0.0, 2.0, 1.4, Straw);
    b.pup(26.0, 0.0);
    b.pup(26.0, t);
    b.frame(30.0, 0.0, 2.0, 1.4, Straw);
    b.pup(30.0, 0.0);
    b.frame(34.0, 0.0, 2.0, 1.4, Wood);
    b.pup(34.0, 0.0);
    b.build("Piggy Pile", vec![Litter, Litter, Grunt])
}

/// Introduces ice and the Thumper (slam).
fn thin_ice() -> Level {
    let mut b = B::default();
    let t = b.frame(28.0, 0.0, 2.6, 2.0, Ice);
    b.wolf(28.0, 0.0, 0.5);
    let t = b.frame(28.0, t, 2.6, 2.0, Ice);
    b.wolf(28.0, t - 2.4, 0.5);
    let t = b.frame(28.0, t, 2.2, 1.6, Ice);
    b.pup(28.0, t - 2.0);
    b.wolf(28.0, t, 0.5);

    let t = b.cube(33.5, 0.0, 1.0, Ice);
    let t = b.cube(33.5, t, 1.0, Ice);
    b.pup(33.5, t);
    b.build("Thin Ice", vec![Thumper, Grunt, Thumper])
}

/// Introduces TNT and helmet wolves.
fn powder_keg() -> Level {
    let mut b = B::default();
    let t = b.frame(27.0, 0.0, 3.6, 2.2, Wood);
    b.tnt(26.1, 0.0);
    b.helmet(27.6, 0.0);
    let t2 = b.frame(27.0, t, 3.0, 2.0, Wood);
    b.wolf(27.0, t, 0.5);
    b.pup(27.0, t2);

    b.tnt(30.6, 0.0);
    b.tnt(31.4, 0.0);
    b.helmet(33.0, 0.0);
    b.build("Powder Keg", vec![Grunt, Dasher, Boomer])
}

/// Introduces boulders, ramps and the Puffer.
fn rock_and_roll() -> Level {
    let mut b = B::default();
    let top = b.hill(22.0, 4.0, 4.0);
    b.boulder(23.0, top, 0.8);
    b.pup(20.8, top);
    b.ramp(24.0, top, 30.0, 0.0);

    let t = b.frame(32.8, 0.0, 3.0, 1.8, Straw);
    b.wolf(32.8, 0.0, 0.5);
    b.wolf(32.8, t, 0.5);
    b.frame(36.4, 0.0, 2.2, 1.6, Wood);
    b.pup(36.4, 0.0);
    b.build("Rock and Roll", vec![Puffer, Grunt, Hog])
}

/// A brick bunker full of helmets: pry it open with the Puffer.
fn tin_hats() -> Level {
    let mut b = B::default();
    let t = b.frame(29.0, 0.0, 4.0, 2.0, Brick);
    b.helmet(28.1, 0.0);
    b.helmet(29.9, 0.0);
    let t2 = b.frame(29.0, t, 3.0, 1.8, Wood);
    b.helmet(29.0, t);
    let t3 = b.cube(29.0, t2, 0.8, Straw);
    b.pup(29.0, t3);

    b.post(24.6, 0.0, 2.4, Brick);
    b.post(25.0, 0.0, 2.4, Brick);
    b.build("Tin Hats", vec![Puffer, Hog, Boomer, Dasher])
}

/// Two towers joined by a bridge loaded with TNT.
fn twin_peaks() -> Level {
    let mut b = B::default();
    for x in [25.0, 33.0] {
        let t = b.frame(x, 0.0, 2.4, 3.0, Wood);
        b.wolf(x, 0.0, 0.5);
        b.frame(x, t, 2.4, 1.6, Ice);
        b.pup(x, t);
    }
    let bridge_y = 3.4 + 2.0;
    let deck = b.plank(29.0, bridge_y, 10.4, Wood);
    b.tnt(27.4, deck);
    b.helmet(29.0, deck);
    b.tnt(30.6, deck);
    b.build("Twin Peaks", vec![Litter, Thumper, Boomer, Grunt])
}

/// The Big Bad Wolf in his keep, with TNT in the cellar and a boulder above.
fn big_bad_boss() -> Level {
    let mut b = B::default();
    let top = b.hill(22.5, 3.0, 3.0);
    b.boulder(23.4, top, 0.7);
    b.ramp(24.0, top, 27.2, 0.0);

    let t = b.frame(30.0, 0.0, 5.0, 2.6, Brick);
    b.boss(30.0, 0.0);
    b.tnt(28.3, 0.0);
    b.tnt(31.7, 0.0);
    let t2 = b.frame(30.0, t, 4.0, 2.0, Wood);
    b.helmet(29.2, t);
    b.helmet(30.8, t);
    let t3 = b.frame(30.0, t2, 2.4, 1.6, Ice);
    b.pup(30.0, t2);
    b.wolf(30.0, t3, 0.5);
    b.build("Big Bad Boss", vec![Thumper, Litter, Puffer, Boomer, Hog])
}
