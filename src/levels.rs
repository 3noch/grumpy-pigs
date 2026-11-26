//! Level definitions. Ground top is y = 0; the sling sits at x ≈ 6.

use crate::sim::{
    BlockDef, Level, Material,
    Material::{Brick, Straw, Wood},
    PigKind::{Boomer, Dasher, Grunt, Hog},
};

const T: f32 = 0.4; // thickness of posts and planks

#[derive(Default)]
struct B {
    blocks: Vec<BlockDef>,
    wolves: Vec<(f32, f32, f32)>,
    rocks: Vec<(f32, f32, f32, f32)>,
}

impl B {
    fn rect(&mut self, cx: f32, bottom: f32, w: f32, h: f32, mat: Material) -> f32 {
        self.blocks.push(BlockDef {
            cx,
            cy: bottom + h / 2.0,
            hw: w / 2.0,
            hh: h / 2.0,
            mat,
        });
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
    fn wolf(&mut self, x: f32, y: f32, r: f32) {
        self.wolves.push((x, y + r, r));
    }
    /// Solid terrain from the ground up to `top`. Returns `top`.
    fn hill(&mut self, cx: f32, top: f32, w: f32) -> f32 {
        self.rocks.push((cx, top / 2.0, w / 2.0, top / 2.0));
        top
    }
    fn build(self, name: &'static str, pigs: Vec<crate::sim::PigKind>) -> Level {
        Level {
            name,
            blocks: self.blocks,
            wolves: self.wolves,
            rocks: self.rocks,
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
