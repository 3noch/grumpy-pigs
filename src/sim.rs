//! Game simulation: physics world, damage, pigs, wolves, turn flow.
//! Deliberately free of any rendering code so it can be unit-tested headless.

use rapier2d::prelude::*;

/// Rapier's 2D vector (glam Vec2).
pub type V2 = rapier2d::math::Vector;

pub const GRAVITY: f32 = -12.0;
/// Resting position of the sling pouch, in world meters.
pub const POUCH: (f32, f32) = (6.0, 3.4);
pub const MAX_PULL: f32 = 2.6;
pub const LAUNCH_K: f32 = 9.2;
const DT: f32 = 1.0 / 60.0;
/// No damage while the level settles in after loading.
const GRACE: f32 = 1.2;
/// Velocity changes below this (m/s per step) are harmless.
const DV_THRESHOLD: f32 = 2.0;
/// Wolves knocked out within this many seconds of each other form a combo.
const COMBO_WINDOW: f32 = 1.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Straw,
    Wood,
    Brick,
    /// Slippery and brittle.
    Ice,
    /// Explodes when destroyed.
    Tnt,
    /// Boulders: heavy, round, nearly indestructible.
    Stone,
}

impl Material {
    fn density(self) -> f32 {
        match self {
            Material::Straw => 0.4,
            Material::Wood | Material::Tnt => 1.0,
            Material::Ice => 0.9,
            Material::Brick => 2.6,
            Material::Stone => 3.0,
        }
    }
    fn hp(self) -> f32 {
        match self {
            Material::Straw => 3.5,
            Material::Wood => 8.0,
            Material::Brick => 26.0,
            Material::Ice => 2.2,
            Material::Tnt => 2.0,
            Material::Stone => 60.0,
        }
    }
    fn friction(self) -> f32 {
        match self {
            Material::Ice => 0.06,
            Material::Stone => 0.9,
            _ => 0.7,
        }
    }
    pub fn points(self) -> u32 {
        match self {
            Material::Straw => 100,
            Material::Ice => 150,
            Material::Wood => 300,
            Material::Tnt => 500,
            Material::Brick => 600,
            Material::Stone => 800,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PigKind {
    /// Plain and furious.
    Grunt,
    /// Tap mid-air to rocket forward.
    Dasher,
    /// Tap (or wait after impact) to blow up.
    Boomer,
    /// Big, heavy, unstoppable.
    Hog,
    /// Tap to split into three piglets.
    Litter,
    /// One of the Litter's piglets.
    Piglet,
    /// Tap to stop dead and slam straight down.
    Thumper,
    /// Tap to puff up to twice the size, shoving everything aside.
    Puffer,
}

impl PigKind {
    pub fn radius(self) -> f32 {
        match self {
            PigKind::Hog => 0.7,
            PigKind::Thumper => 0.55,
            PigKind::Dasher | PigKind::Litter => 0.4,
            PigKind::Piglet => 0.27,
            _ => 0.45,
        }
    }
    fn density(self) -> f32 {
        match self {
            PigKind::Hog => 6.0,
            PigKind::Thumper => 7.0,
            PigKind::Boomer => 3.5,
            PigKind::Piglet => 4.0,
            PigKind::Puffer => 2.5,
            _ => 3.0,
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            PigKind::Grunt | PigKind::Hog | PigKind::Piglet => "",
            PigKind::Dasher => "Tap to DASH!",
            PigKind::Boomer => "Tap to BOOM!",
            PigKind::Litter => "Tap to SPLIT!",
            PigKind::Thumper => "Tap to SLAM!",
            PigKind::Puffer => "Tap to PUFF!",
        }
    }
    /// Whether tapping mid-flight does something.
    pub fn has_power(self) -> bool {
        !self.hint().is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WolfKind {
    /// Small and fragile.
    Pup,
    Wolf,
    /// Wears a cooking pot: twice as tough.
    Helmet,
    /// The Big Bad Wolf.
    Boss,
}

impl WolfKind {
    pub fn radius(self) -> f32 {
        match self {
            WolfKind::Pup => 0.38,
            WolfKind::Wolf => 0.52,
            WolfKind::Helmet => 0.55,
            WolfKind::Boss => 0.95,
        }
    }
    fn hp(self) -> f32 {
        match self {
            WolfKind::Pup => 3.0,
            WolfKind::Wolf => 6.0,
            WolfKind::Helmet => 13.0,
            WolfKind::Boss => 24.0,
        }
    }
    pub fn points(self) -> u32 {
        match self {
            WolfKind::Pup => 3000,
            WolfKind::Wolf => 5000,
            WolfKind::Helmet => 7000,
            WolfKind::Boss => 15000,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Block(Material),
    Wolf(WolfKind),
    Pig(PigKind),
    /// Immovable terrain.
    Rock,
}

impl Kind {
    pub fn is_wolf(self) -> bool {
        matches!(self, Kind::Wolf(_))
    }
    pub fn is_pig(self) -> bool {
        matches!(self, Kind::Pig(_))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box { hw: f32, hh: f32 },
    Ball { r: f32 },
}

pub struct Ent {
    pub handle: RigidBodyHandle,
    col: ColliderHandle,
    pub kind: Kind,
    pub shape: Shape,
    pub hp: f32,
    pub max_hp: f32,
    pub pos: V2,
    pub angle: f32,
    pub hurt_flash: f32,
    /// Current linear velocity (for animation).
    pub vel: V2,
    /// Recent impact strength 0..1, decays quickly (for squash animation).
    pub impact: f32,
    /// Velocity direction just before the last impact (squash axis).
    pub impact_dir: V2,
    pub alive: bool,
    /// Stable per-entity seed used for cosmetic variation (cracks etc).
    pub seed: u32,
    /// Puffer inflation progress 0..1, once triggered.
    pub puff: Option<f32>,
    prev_v: V2,
}

impl Ent {
    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }
    /// Current radius of a round entity (0 for boxes).
    pub fn radius(&self) -> f32 {
        match self.shape {
            Shape::Ball { r } => r,
            Shape::Box { .. } => 0.0,
        }
    }
}

/// The pig currently in the air.
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    pub ent: usize,
    pub kind: PigKind,
    pub used: bool,
    /// Seconds since it first hit something.
    pub since_impact: Option<f32>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Destroyed {
        pos: V2,
        kind: Kind,
        points: u32,
    },
    Explosion {
        pos: V2,
        radius: f32,
    },
    Hit {
        pos: V2,
        strength: f32,
    },
    /// A pig used its power.
    Power {
        pos: V2,
        kind: PigKind,
    },
    /// Several wolves knocked out in quick succession.
    Combo {
        pos: V2,
        count: u32,
        bonus: u32,
    },
    Launch,
    Poof {
        pos: V2,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub cx: f32,
    pub cy: f32,
    pub hw: f32,
    pub hh: f32,
    pub mat: Material,
}

#[derive(Clone, Copy, Debug)]
pub struct WolfDef {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub kind: WolfKind,
}

/// Fixed terrain: a box, optionally tilted into a ramp.
#[derive(Clone, Copy, Debug)]
pub struct RockDef {
    pub cx: f32,
    pub cy: f32,
    pub hw: f32,
    pub hh: f32,
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub name: &'static str,
    pub blocks: Vec<BlockDef>,
    pub wolves: Vec<WolfDef>,
    pub rocks: Vec<RockDef>,
    /// Round stone boulders: (x, y, radius).
    pub boulders: Vec<(f32, f32, f32)>,
    pub pigs: Vec<PigKind>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Aiming,
    Flying {
        ent: usize,
        t: f32,
        slow_t: f32,
        used: bool,
        impact_t: Option<f32>,
    },
    Settling {
        t: f32,
    },
    Won {
        t: f32,
    },
    Lost {
        t: f32,
    },
}

pub struct Game {
    world: PhysicsWorld,
    pub ents: Vec<Ent>,
    pub pigs_left: Vec<PigKind>,
    pub phase: Phase,
    pub score: u32,
    pub max_score: u32,
    pub time: f32,
    pub events: Vec<Event>,
    /// Current sling pull (offset from pouch), set by input while aiming.
    pub aim: Option<V2>,
    acc: f32,
    /// Explosions queued by destroyed TNT, set off after this step's removals.
    blasts: Vec<V2>,
    combo: u32,
    last_ko: f32,
}

fn rotate(v: V2, a: f32) -> V2 {
    let (s, c) = a.sin_cos();
    V2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

impl Game {
    pub fn new(level: &Level) -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = V2::new(0.0, GRAVITY);
        world.integration_parameters.num_solver_iterations = 8;

        let mut g = Game {
            world,
            ents: Vec::new(),
            pigs_left: level.pigs.clone(),
            phase: Phase::Aiming,
            score: 0,
            max_score: 0,
            time: 0.0,
            events: Vec::new(),
            aim: None,
            acc: 0.0,
            blasts: Vec::new(),
            combo: 0,
            last_ko: f32::NEG_INFINITY,
        };

        // Ground: top surface at y = 0.
        let ground = RigidBodyBuilder::fixed().translation(V2::new(20.0, -5.0));
        g.world
            .insert(ground, ColliderBuilder::cuboid(200.0, 5.0).friction(0.9));

        for r in &level.rocks {
            let body = RigidBodyBuilder::fixed()
                .translation(V2::new(r.cx, r.cy))
                .rotation(r.angle);
            let col = ColliderBuilder::cuboid(r.hw, r.hh).friction(0.9);
            let (h, c) = g.world.insert(body, col);
            let shape = Shape::Box { hw: r.hw, hh: r.hh };
            g.push(h, c, Kind::Rock, shape, f32::INFINITY);
        }
        for b in &level.blocks {
            let body = RigidBodyBuilder::dynamic().translation(V2::new(b.cx, b.cy));
            let col = ColliderBuilder::cuboid(b.hw, b.hh)
                .density(b.mat.density())
                .friction(b.mat.friction())
                .restitution(0.05);
            let (h, c) = g.world.insert(body, col);
            let shape = Shape::Box { hw: b.hw, hh: b.hh };
            g.push(h, c, Kind::Block(b.mat), shape, b.mat.hp());
            g.max_score += b.mat.points();
        }
        for &(x, y, r) in &level.boulders {
            let body = RigidBodyBuilder::dynamic()
                .translation(V2::new(x, y))
                .angular_damping(0.1);
            let col = ColliderBuilder::ball(r)
                .density(Material::Stone.density())
                .friction(Material::Stone.friction())
                .restitution(0.1);
            let (h, c) = g.world.insert(body, col);
            let kind = Kind::Block(Material::Stone);
            g.push(h, c, kind, Shape::Ball { r }, Material::Stone.hp());
        }
        for w in &level.wolves {
            let body = RigidBodyBuilder::dynamic()
                .translation(V2::new(w.x, w.y))
                .angular_damping(1.0);
            let col = ColliderBuilder::ball(w.r)
                .density(1.0)
                .friction(0.8)
                .restitution(0.1);
            let (h, c) = g.world.insert(body, col);
            g.push(
                h,
                c,
                Kind::Wolf(w.kind),
                Shape::Ball { r: w.r },
                w.kind.hp(),
            );
            g.max_score += w.kind.points();
        }
        g.max_score += 10_000 * u32::try_from(level.pigs.len().saturating_sub(1)).unwrap_or(0);
        g
    }

    fn push(
        &mut self,
        handle: RigidBodyHandle,
        col: ColliderHandle,
        kind: Kind,
        shape: Shape,
        hp: f32,
    ) -> usize {
        let body = &self.world.bodies[handle];
        let seed = u32::try_from(self.ents.len())
            .unwrap_or(0)
            .wrapping_mul(2_654_435_761)
            .rotate_left(7);
        self.ents.push(Ent {
            handle,
            col,
            kind,
            shape,
            hp,
            max_hp: hp,
            pos: body.translation(),
            angle: body.rotation().angle(),
            hurt_flash: 0.0,
            vel: body.linvel(),
            impact: 0.0,
            impact_dir: V2::X,
            alive: true,
            seed,
            puff: None,
            prev_v: body.linvel(),
        });
        self.ents.len() - 1
    }

    pub fn wolves_alive(&self) -> usize {
        self.ents
            .iter()
            .filter(|e| e.alive && e.kind.is_wolf())
            .count()
    }

    pub fn current_pig(&self) -> Option<PigKind> {
        match self.phase {
            Phase::Aiming => self.pigs_left.first().copied(),
            _ => None,
        }
    }

    pub fn flying_pig(&self) -> Option<(PigKind, bool)> {
        self.flight().map(|f| (f.kind, f.used))
    }

    /// Details about the pig currently in the air, if any.
    pub fn flight(&self) -> Option<Flight> {
        if let Phase::Flying {
            ent,
            used,
            impact_t,
            ..
        } = self.phase
            && let Kind::Pig(kind) = self.ents[ent].kind
        {
            return Some(Flight {
                ent,
                kind,
                used,
                since_impact: impact_t,
            });
        }
        None
    }

    pub fn pouch() -> V2 {
        V2::new(POUCH.0, POUCH.1)
    }

    pub fn launch_velocity(pull: V2) -> V2 {
        -pull * LAUNCH_K
    }

    /// Sample points of the predicted flight path for a given pull.
    pub fn trajectory(pull: V2, n: usize, step: f32) -> Vec<V2> {
        let p0 = Self::pouch() + pull;
        let v = Self::launch_velocity(pull);
        (1..=n)
            .map(|i| {
                let t = i as f32 * step;
                p0 + v * t + V2::new(0.0, 0.5 * GRAVITY * t * t)
            })
            .collect()
    }

    fn spawn_pig(&mut self, kind: PigKind, pos: V2, vel: V2) -> usize {
        let r = kind.radius();
        let body = RigidBodyBuilder::dynamic()
            .translation(pos)
            .linvel(vel)
            .ccd_enabled(true)
            .angular_damping(1.5);
        let col = ColliderBuilder::ball(r)
            .density(kind.density())
            .friction(0.6)
            .restitution(0.3);
        let (h, c) = self.world.insert(body, col);
        self.push(h, c, Kind::Pig(kind), Shape::Ball { r }, f32::INFINITY)
    }

    /// Fire the current pig. `pull` is the offset from the pouch.
    pub fn launch(&mut self, pull: V2) {
        if self.phase != Phase::Aiming || self.pigs_left.is_empty() {
            return;
        }
        let pull = pull.clamp_length_max(MAX_PULL);
        let kind = self.pigs_left.remove(0);
        let idx = self.spawn_pig(kind, Self::pouch() + pull, Self::launch_velocity(pull));
        self.aim = None;
        self.phase = Phase::Flying {
            ent: idx,
            t: 0.0,
            slow_t: 0.0,
            used: false,
            impact_t: None,
        };
        self.events.push(Event::Launch);
    }

    /// Trigger the flying pig's special ability (tap mid-flight).
    pub fn ability(&mut self) {
        let Phase::Flying {
            ent, used: false, ..
        } = self.phase
        else {
            return;
        };
        let Kind::Pig(kind) = self.ents[ent].kind else {
            return;
        };
        if !kind.has_power() {
            return;
        }
        let pos = self.ents[ent].pos;
        let handle = self.ents[ent].handle;
        let v = self.world.bodies[handle].linvel();
        match kind {
            PigKind::Dasher => {
                let dir = v.normalize_or(V2::X);
                let speed = (v.length() * 1.8).max(24.0);
                self.world.bodies[handle].set_linvel(dir * speed, true);
                // A power-up isn't a collision: don't count it as an impact.
                self.ents[ent].prev_v = dir * speed;
            }
            PigKind::Boomer => {
                self.explode_at(pos, 3.8, 1.0, Some(ent));
                self.kill(ent, false);
            }
            PigKind::Litter => {
                // The leader shrinks into a piglet; two siblings fan out beside it.
                let r = PigKind::Piglet.radius();
                self.world.colliders[self.ents[ent].col].set_shape(SharedShape::ball(r));
                self.ents[ent].kind = Kind::Pig(PigKind::Piglet);
                self.ents[ent].shape = Shape::Ball { r };
                let side = rotate(v.normalize_or(V2::X), std::f32::consts::FRAC_PI_2);
                for (a, off) in [(0.2_f32, 0.7_f32), (-0.2, -0.7)] {
                    self.spawn_pig(PigKind::Piglet, pos + side * off, rotate(v, a));
                }
            }
            PigKind::Thumper => {
                let body = &mut self.world.bodies[handle];
                let slam = V2::new(v.x * 0.1, -32.0);
                body.set_linvel(slam, true);
                body.set_angvel(0.0, true);
                self.ents[ent].prev_v = slam;
            }
            PigKind::Puffer => {
                self.ents[ent].puff = Some(0.0);
            }
            PigKind::Grunt | PigKind::Hog | PigKind::Piglet => {}
        }
        self.events.push(Event::Power { pos, kind });
        if let Phase::Flying { used, .. } = &mut self.phase {
            *used = true;
        }
    }

    /// Radial blast: shoves bodies outward and damages them.
    fn explode_at(&mut self, center: V2, radius: f32, power: f32, skip: Option<usize>) {
        self.events.push(Event::Explosion {
            pos: center,
            radius,
        });
        for i in 0..self.ents.len() {
            let e = &self.ents[i];
            if !e.alive || Some(i) == skip || e.kind == Kind::Rock {
                continue;
            }
            let d = e.pos - center;
            let dist = d.length();
            if dist > radius {
                continue;
            }
            let falloff = 1.0 - dist / radius;
            let dir = if dist > 0.01 { d / dist } else { V2::Y };
            let body = &mut self.world.bodies[e.handle];
            let impulse = dir * body.mass() * 22.0 * falloff * power;
            body.apply_impulse(impulse, true);
            let e = &mut self.ents[i];
            if !e.kind.is_pig() {
                e.hp -= 11.0 * falloff * power;
                e.hurt_flash = 1.0;
            }
        }
    }

    fn kill(&mut self, i: usize, award: bool) {
        let e = &mut self.ents[i];
        if !e.alive {
            return;
        }
        e.alive = false;
        e.hp = e.hp.min(0.0);
        let (pos, kind) = (e.pos, e.kind);
        self.world.remove_body(e.handle);
        let points = match kind {
            _ if !award => 0,
            Kind::Block(m) => m.points(),
            Kind::Wolf(w) => w.points(),
            _ => 0,
        };
        self.score += points;
        self.events.push(match kind {
            Kind::Pig(_) => Event::Poof { pos },
            kind => Event::Destroyed { pos, kind, points },
        });
        if kind == Kind::Block(Material::Tnt) {
            self.blasts.push(pos);
        }
        if award && kind.is_wolf() {
            self.combo = if self.time - self.last_ko < COMBO_WINDOW {
                self.combo + 1
            } else {
                1
            };
            self.last_ko = self.time;
            if self.combo >= 2 {
                let bonus = 2500 * (self.combo - 1);
                self.score += bonus;
                self.events.push(Event::Combo {
                    pos,
                    count: self.combo,
                    bonus,
                });
            }
        }
    }

    /// Advance the simulation by a frame's worth of real time.
    pub fn update(&mut self, frame_dt: f32) {
        self.acc += frame_dt.min(0.1);
        while self.acc >= DT {
            self.acc -= DT;
            self.fixed_step();
        }
    }

    /// Grow inflating Puffers a little each step.
    fn step_puffers(&mut self) {
        for e in self.ents.iter_mut().filter(|e| e.alive) {
            let (Some(p), Kind::Pig(kind)) = (e.puff, e.kind) else {
                continue;
            };
            if p >= 1.0 {
                continue;
            }
            let p = (p + DT / 0.2).min(1.0);
            e.puff = Some(p);
            let ease = 1.0 - (1.0 - p) * (1.0 - p);
            let r = kind.radius() * (1.0 + 1.2 * ease);
            e.shape = Shape::Ball { r };
            self.world.colliders[e.col].set_shape(SharedShape::ball(r));
            self.world.bodies[e.handle].wake_up(true);
        }
    }

    fn fixed_step(&mut self) {
        self.time += DT;
        if matches!(self.phase, Phase::Won { .. } | Phase::Lost { .. }) {
            // Keep things tumbling in the background but stop scoring changes.
            self.world.step();
            self.sync_positions();
            self.advance_phase();
            return;
        }
        self.step_puffers();
        self.world.step();

        let damage_on = self.time > GRACE;
        let flying = match self.phase {
            Phase::Flying { ent, .. } => Some(ent),
            _ => None,
        };
        let mut pig_impact = false;

        for i in 0..self.ents.len() {
            let e = &mut self.ents[i];
            if !e.alive || e.kind == Kind::Rock {
                continue;
            }
            let body = &self.world.bodies[e.handle];
            e.pos = body.translation();
            e.angle = body.rotation().angle();
            let v = body.linvel();
            let dv = (v - e.prev_v).length();
            if dv > 2.5 {
                let strength = (dv / 12.0).min(1.0);
                if strength > e.impact {
                    e.impact = strength;
                    e.impact_dir = (v - e.prev_v).normalize_or(V2::Y);
                }
            }
            e.prev_v = v;
            e.vel = v;
            e.impact = (e.impact - DT * 2.5).max(0.0);
            e.hurt_flash = (e.hurt_flash - DT * 3.0).max(0.0);

            if e.kind.is_pig() {
                if Some(i) == flying && dv > 3.0 {
                    pig_impact = true;
                }
            } else if damage_on && dv > DV_THRESHOLD {
                let dmg = dv - DV_THRESHOLD;
                e.hp -= dmg;
                e.hurt_flash = 1.0;
                if dmg > 2.0 {
                    let pos = e.pos;
                    self.events.push(Event::Hit { pos, strength: dmg });
                }
            }
        }

        // Remove destroyed / out-of-bounds entities.
        for i in 0..self.ents.len() {
            let e = &self.ents[i];
            if !e.alive || e.kind == Kind::Rock {
                continue;
            }
            let oob = e.pos.y < -4.0 || e.pos.x < -8.0 || e.pos.x > 60.0;
            if e.hp <= 0.0 || (oob && !e.kind.is_pig()) {
                self.kill(i, true);
            }
        }
        // TNT goes off after removals; anything it destroys is removed next
        // step, which lets chain reactions ripple outward.
        for center in std::mem::take(&mut self.blasts) {
            self.explode_at(center, 3.4, 1.1, None);
        }

        if let Phase::Flying { impact_t, .. } = &mut self.phase
            && pig_impact
            && impact_t.is_none()
        {
            *impact_t = Some(0.0);
        }
        self.advance_phase();
    }

    fn sync_positions(&mut self) {
        for e in self.ents.iter_mut().filter(|e| e.alive) {
            let body = &self.world.bodies[e.handle];
            e.pos = body.translation();
            e.angle = body.rotation().angle();
        }
    }

    fn world_is_calm(&self) -> bool {
        self.ents
            .iter()
            .filter(|e| e.alive && e.kind != Kind::Rock)
            .all(|e| {
                let b = &self.world.bodies[e.handle];
                b.linvel().length() < 0.25 && b.angvel().abs() < 0.3
            })
    }

    /// Ends the current shot: every pig still on the field poofs away.
    fn end_flight(&mut self) {
        for i in 0..self.ents.len() {
            if self.ents[i].alive && self.ents[i].kind.is_pig() {
                self.kill(i, false);
            }
        }
        self.phase = Phase::Settling { t: 0.0 };
    }

    fn advance_phase(&mut self) {
        match self.phase {
            Phase::Aiming => {}
            Phase::Flying {
                ent,
                t,
                slow_t,
                used,
                impact_t,
            } => {
                let t = t + DT;
                let e = &self.ents[ent];
                if !e.alive {
                    self.end_flight();
                    return;
                }
                let speed = self.world.bodies[e.handle].linvel().length();
                let slow_t = if speed < 0.6 { slow_t + DT } else { 0.0 };
                let impact_t = impact_t.map(|x| x + DT);
                let oob = e.pos.x > 60.0 || e.pos.x < -8.0 || e.pos.y < -4.0;
                let is_boomer = e.kind == Kind::Pig(PigKind::Boomer);

                if is_boomer && !used && impact_t.is_some_and(|x| x > 1.2) {
                    let pos = e.pos;
                    self.explode_at(pos, 3.8, 1.0, Some(ent));
                    self.end_flight();
                    return;
                }
                if oob
                    || slow_t > 0.8
                    || t > 9.0
                    || impact_t.is_some_and(|x| x > 3.5)
                    || self.wolves_alive() == 0
                {
                    self.end_flight();
                } else {
                    self.phase = Phase::Flying {
                        ent,
                        t,
                        slow_t,
                        used,
                        impact_t,
                    };
                }
            }
            Phase::Settling { t } => {
                let t = t + DT;
                let done = t > 0.8 && (self.world_is_calm() || t > 4.0);
                if !done {
                    self.phase = Phase::Settling { t };
                } else if self.wolves_alive() == 0 {
                    self.score += 10_000 * u32::try_from(self.pigs_left.len()).unwrap_or(0);
                    self.phase = Phase::Won { t: 0.0 };
                } else if !self.pigs_left.is_empty() {
                    self.phase = Phase::Aiming;
                } else {
                    self.phase = Phase::Lost { t: 0.0 };
                }
            }
            Phase::Won { t } => self.phase = Phase::Won { t: t + DT },
            Phase::Lost { t } => self.phase = Phase::Lost { t: t + DT },
        }
    }

    /// 1–3 stars based on share of the maximum achievable score.
    pub fn stars(&self) -> u8 {
        let f = self.score as f32 / self.max_score.max(1) as f32;
        if f >= 0.55 {
            3
        } else if f >= 0.35 {
            2
        } else {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels;

    /// Advance `frames` fixed steps (60 per second).
    fn run(g: &mut Game, frames: u32) {
        for _ in 0..frames {
            g.update(DT);
        }
    }

    #[test]
    fn levels_settle_without_damage() {
        for lvl in levels::all() {
            let mut g = Game::new(&lvl);
            let before = g.ents.iter().filter(|e| e.alive).count();
            run(&mut g, 240);
            let after = g.ents.iter().filter(|e| e.alive).count();
            assert_eq!(before, after, "level '{}' collapsed on its own", lvl.name);
            assert_eq!(g.phase, Phase::Aiming);
        }
    }

    #[test]
    fn a_good_shot_does_damage() {
        let lvl = &levels::all()[0];
        let mut g = Game::new(lvl);
        run(&mut g, 90);
        // Try a fan of shots; at least one should kill something.
        let mut best = 0;
        for i in 0..12 {
            let mut g = Game::new(lvl);
            run(&mut g, 90);
            let ang = (10.0 + i as f32 * 5.0).to_radians();
            g.launch(V2::new(-ang.cos(), -ang.sin()) * MAX_PULL);
            run(&mut g, 480);
            best = best.max(g.score);
        }
        assert!(best >= 5000, "no shot killed a wolf (best score {best})");
    }

    /// A test shot: `(angle_deg, power 0..1, ability tap delay)`.
    type Shot = (f32, f32, Option<f32>);

    /// Replays shots on a fresh copy of the level.
    fn replay(lvl: &Level, shots: &[Shot]) -> Game {
        let mut g = Game::new(lvl);
        run(&mut g, 90);
        for &(deg, pow, tap) in shots {
            if g.phase != Phase::Aiming {
                break;
            }
            let a = deg.to_radians();
            g.launch(V2::new(-a.cos(), -a.sin()) * MAX_PULL * pow);
            let mut t = 0.0;
            while matches!(g.phase, Phase::Flying { .. } | Phase::Settling { .. }) && t < 20.0 {
                if tap.is_some_and(|d| (t - d).abs() < DT / 2.0) {
                    g.ability();
                }
                g.update(DT);
                t += DT;
            }
        }
        g
    }

    /// Greedy bot: every level must be beatable.
    #[test]
    fn every_level_is_winnable() {
        for lvl in levels::all() {
            let mut shots: Vec<Shot> = Vec::new();
            let mut won = false;
            for p in 0..lvl.pigs.len() {
                let has_power = lvl.pigs[p].has_power();
                let taps: &[Option<f32>] = if has_power {
                    &[None, Some(0.6), Some(1.0), Some(1.4)]
                } else {
                    &[None]
                };
                let mut best: Option<((usize, u32), Shot)> = None;
                for deg in (-10..=70).step_by(4) {
                    for pow in [0.7, 0.8, 0.9, 1.0] {
                        for &tap in taps {
                            let mut s = shots.clone();
                            s.push((deg as f32, pow, tap));
                            let g = replay(&lvl, &s);
                            let key = (usize::MAX - g.wolves_alive(), g.score);
                            if best.as_ref().is_none_or(|(k, _)| key > *k) {
                                best = Some((key, (deg as f32, pow, tap)));
                            }
                        }
                    }
                }
                shots.push(best.unwrap().1);
                let g = replay(&lvl, &shots);
                eprintln!(
                    "{}: pig {} -> {:?}, wolves left {}, score {}",
                    lvl.name,
                    p,
                    shots.last().unwrap(),
                    g.wolves_alive(),
                    g.score
                );
                if matches!(g.phase, Phase::Won { .. }) {
                    won = true;
                    eprintln!("  WON with {} stars", g.stars());
                    break;
                }
            }
            assert!(won, "bot could not beat '{}'", lvl.name);
        }
    }

    #[test]
    fn turn_flow_reaches_end() {
        let lvl = &levels::all()[0];
        let mut g = Game::new(lvl);
        run(&mut g, 90);
        for _ in 0..lvl.pigs.len() {
            // Weak shot straight down into the ground.
            g.launch(V2::new(-0.5, 0.5));
            run(&mut g, 900);
        }
        assert!(matches!(g.phase, Phase::Lost { .. } | Phase::Won { .. }));
    }
}
