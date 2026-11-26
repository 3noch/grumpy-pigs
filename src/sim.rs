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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Straw,
    Wood,
    Brick,
}

impl Material {
    fn density(self) -> f32 {
        match self {
            Material::Straw => 0.4,
            Material::Wood => 1.0,
            Material::Brick => 2.6,
        }
    }
    fn hp(self) -> f32 {
        match self {
            Material::Straw => 3.5,
            Material::Wood => 8.0,
            Material::Brick => 26.0,
        }
    }
    pub fn points(self) -> u32 {
        match self {
            Material::Straw => 100,
            Material::Wood => 300,
            Material::Brick => 600,
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
}

impl PigKind {
    pub fn radius(self) -> f32 {
        match self {
            PigKind::Hog => 0.7,
            PigKind::Dasher => 0.4,
            _ => 0.45,
        }
    }
    fn density(self) -> f32 {
        match self {
            PigKind::Hog => 6.0,
            PigKind::Boomer => 3.5,
            _ => 3.0,
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            PigKind::Grunt => "",
            PigKind::Dasher => "Tap to DASH!",
            PigKind::Boomer => "Tap to BOOM!",
            PigKind::Hog => "",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Block(Material),
    Wolf,
    Pig(PigKind),
    /// Immovable terrain.
    Rock,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box { hw: f32, hh: f32 },
    Ball { r: f32 },
}

pub struct Ent {
    pub handle: RigidBodyHandle,
    pub kind: Kind,
    pub shape: Shape,
    pub hp: f32,
    pub max_hp: f32,
    pub pos: V2,
    pub angle: f32,
    pub hurt_flash: f32,
    pub alive: bool,
    /// Stable per-entity seed used for cosmetic variation (cracks etc).
    pub seed: u32,
    prev_v: V2,
}

impl Ent {
    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Destroyed { pos: V2, kind: Kind, points: u32 },
    Explosion { pos: V2, radius: f32 },
    Hit { pos: V2, strength: f32 },
    Dash { pos: V2 },
    Launch,
    Poof { pos: V2 },
}

#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub cx: f32,
    pub cy: f32,
    pub hw: f32,
    pub hh: f32,
    pub mat: Material,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub name: &'static str,
    pub blocks: Vec<BlockDef>,
    /// (x, y, radius)
    pub wolves: Vec<(f32, f32, f32)>,
    /// Fixed terrain boxes: (cx, cy, hw, hh)
    pub rocks: Vec<(f32, f32, f32, f32)>,
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
        };

        // Ground: top surface at y = 0.
        let ground = RigidBodyBuilder::fixed().translation(V2::new(20.0, -5.0));
        g.world
            .insert(ground, ColliderBuilder::cuboid(200.0, 5.0).friction(0.9));

        for &(cx, cy, hw, hh) in &level.rocks {
            let body = RigidBodyBuilder::fixed().translation(V2::new(cx, cy));
            let (h, _) = g
                .world
                .insert(body, ColliderBuilder::cuboid(hw, hh).friction(0.9));
            g.push(h, Kind::Rock, Shape::Box { hw, hh }, f32::INFINITY);
        }
        for b in &level.blocks {
            let body = RigidBodyBuilder::dynamic().translation(V2::new(b.cx, b.cy));
            let col = ColliderBuilder::cuboid(b.hw, b.hh)
                .density(b.mat.density())
                .friction(0.7)
                .restitution(0.05);
            let (h, _) = g.world.insert(body, col);
            g.push(
                h,
                Kind::Block(b.mat),
                Shape::Box { hw: b.hw, hh: b.hh },
                b.mat.hp(),
            );
            g.max_score += b.mat.points();
        }
        for &(x, y, r) in &level.wolves {
            let body = RigidBodyBuilder::dynamic()
                .translation(V2::new(x, y))
                .angular_damping(1.0);
            let col = ColliderBuilder::ball(r)
                .density(1.0)
                .friction(0.8)
                .restitution(0.1);
            let (h, _) = g.world.insert(body, col);
            g.push(h, Kind::Wolf, Shape::Ball { r }, 6.0);
            g.max_score += 5000;
        }
        g.max_score += 10_000 * (level.pigs.len().saturating_sub(1)) as u32;
        g
    }

    fn push(&mut self, handle: RigidBodyHandle, kind: Kind, shape: Shape, hp: f32) -> usize {
        let body = &self.world.bodies[handle];
        let seed = (self.ents.len() as u32)
            .wrapping_mul(2654435761)
            .rotate_left(7);
        self.ents.push(Ent {
            handle,
            kind,
            shape,
            hp,
            max_hp: hp,
            pos: body.translation(),
            angle: body.rotation().angle(),
            hurt_flash: 0.0,
            alive: true,
            seed,
            prev_v: V2::ZERO,
        });
        self.ents.len() - 1
    }

    pub fn wolves_alive(&self) -> usize {
        self.ents
            .iter()
            .filter(|e| e.alive && e.kind == Kind::Wolf)
            .count()
    }

    pub fn current_pig(&self) -> Option<PigKind> {
        match self.phase {
            Phase::Aiming => self.pigs_left.first().copied(),
            _ => None,
        }
    }

    pub fn flying_pig(&self) -> Option<(PigKind, bool)> {
        if let Phase::Flying { ent, used, .. } = self.phase
            && let Kind::Pig(k) = self.ents[ent].kind
        {
            return Some((k, used));
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

    /// Fire the current pig. `pull` is the offset from the pouch.
    pub fn launch(&mut self, pull: V2) {
        if self.phase != Phase::Aiming || self.pigs_left.is_empty() {
            return;
        }
        let pull = pull.clamp_length_max(MAX_PULL);
        let kind = self.pigs_left.remove(0);
        let r = kind.radius();
        let body = RigidBodyBuilder::dynamic()
            .translation(Self::pouch() + pull)
            .linvel(Self::launch_velocity(pull))
            .ccd_enabled(true)
            .angular_damping(0.6);
        let col = ColliderBuilder::ball(r)
            .density(kind.density())
            .friction(0.6)
            .restitution(0.3);
        let (h, _) = self.world.insert(body, col);
        let idx = self.push(h, Kind::Pig(kind), Shape::Ball { r }, f32::INFINITY);
        self.ents[idx].prev_v = Self::launch_velocity(pull);
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
        match kind {
            PigKind::Dasher => {
                let body = &mut self.world.bodies[self.ents[ent].handle];
                let v = body.linvel();
                let dir = if v.length() > 0.1 {
                    v.normalize()
                } else {
                    V2::X
                };
                let speed = (v.length() * 1.8).max(24.0);
                body.set_linvel(dir * speed, true);
                self.events.push(Event::Dash {
                    pos: self.ents[ent].pos,
                });
            }
            PigKind::Boomer => {
                self.explode(ent);
            }
            PigKind::Grunt | PigKind::Hog => return,
        }
        if let Phase::Flying { used, .. } = &mut self.phase {
            *used = true;
        }
    }

    fn explode(&mut self, pig: usize) {
        let center = self.ents[pig].pos;
        let radius = 3.8;
        self.events.push(Event::Explosion {
            pos: center,
            radius,
        });
        for i in 0..self.ents.len() {
            let e = &self.ents[i];
            if !e.alive || i == pig || e.kind == Kind::Rock {
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
            let impulse = dir * body.mass() * 22.0 * falloff;
            body.apply_impulse(impulse, true);
            let dmg = 11.0 * falloff;
            let e = &mut self.ents[i];
            e.hp -= dmg;
            e.hurt_flash = 1.0;
        }
        self.kill(pig, false);
        self.ents[pig].hp = 0.0;
        if let Phase::Flying { used, .. } = &mut self.phase {
            *used = true;
        }
    }

    fn kill(&mut self, i: usize, award: bool) {
        let e = &mut self.ents[i];
        if !e.alive {
            return;
        }
        e.alive = false;
        let points = if award {
            match e.kind {
                Kind::Block(m) => m.points(),
                Kind::Wolf => 5000,
                _ => 0,
            }
        } else {
            0
        };
        self.score += points;
        let ev = match e.kind {
            Kind::Pig(_) => Event::Poof { pos: e.pos },
            kind => Event::Destroyed {
                pos: e.pos,
                kind,
                points,
            },
        };
        self.events.push(ev);
        self.world.remove_body(e.handle);
    }

    /// Advance the simulation by a frame's worth of real time.
    pub fn update(&mut self, frame_dt: f32) {
        self.acc += frame_dt.min(0.1);
        while self.acc >= DT {
            self.acc -= DT;
            self.fixed_step();
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
            e.prev_v = v;
            e.hurt_flash = (e.hurt_flash - DT * 3.0).max(0.0);

            if matches!(e.kind, Kind::Pig(_)) {
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
            if e.hp <= 0.0 || (oob && !matches!(e.kind, Kind::Pig(_))) {
                self.kill(i, true);
            }
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
                    self.phase = Phase::Settling { t: 0.0 };
                    return;
                }
                let speed = self.world.bodies[e.handle].linvel().length();
                let slow_t = if speed < 0.6 { slow_t + DT } else { 0.0 };
                let impact_t = impact_t.map(|x| x + DT);
                let oob = e.pos.x > 60.0 || e.pos.x < -8.0 || e.pos.y < -4.0;
                let is_boomer = e.kind == Kind::Pig(PigKind::Boomer);

                if is_boomer && !used && impact_t.is_some_and(|x| x > 1.2) {
                    self.explode(ent);
                    self.phase = Phase::Settling { t: 0.0 };
                    return;
                }
                if oob || slow_t > 0.8 || t > 9.0 || self.wolves_alive() == 0 {
                    self.kill(ent, false);
                    self.phase = Phase::Settling { t: 0.0 };
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
                    self.score += 10_000 * self.pigs_left.len() as u32;
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

    fn run(g: &mut Game, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            g.update(DT);
        }
    }

    #[test]
    fn levels_settle_without_damage() {
        for lvl in levels::all() {
            let mut g = Game::new(&lvl);
            let before = g.ents.iter().filter(|e| e.alive).count();
            run(&mut g, 4.0);
            let after = g.ents.iter().filter(|e| e.alive).count();
            assert_eq!(before, after, "level '{}' collapsed on its own", lvl.name);
            assert_eq!(g.phase, Phase::Aiming);
        }
    }

    #[test]
    fn a_good_shot_does_damage() {
        let lvl = &levels::all()[0];
        let mut g = Game::new(lvl);
        run(&mut g, 1.5);
        // Try a fan of shots; at least one should kill something.
        let mut best = 0;
        for i in 0..12 {
            let mut g = Game::new(lvl);
            run(&mut g, 1.5);
            let ang = (10.0 + i as f32 * 5.0).to_radians();
            g.launch(V2::new(-ang.cos(), -ang.sin()) * MAX_PULL);
            run(&mut g, 8.0);
            best = best.max(g.score);
        }
        assert!(best >= 5000, "no shot killed a wolf (best score {best})");
    }

    /// Plays a sequence of (angle_deg, power 0..1, tap_delay) shots from scratch.
    fn replay(lvl: &Level, shots: &[(f32, f32, Option<f32>)]) -> Game {
        let mut g = Game::new(lvl);
        run(&mut g, 1.5);
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
            let mut shots: Vec<(f32, f32, Option<f32>)> = Vec::new();
            let mut won = false;
            for p in 0..lvl.pigs.len() {
                let has_power = matches!(lvl.pigs[p], PigKind::Dasher | PigKind::Boomer);
                let taps: &[Option<f32>] = if has_power {
                    &[None, Some(0.6), Some(1.0), Some(1.4)]
                } else {
                    &[None]
                };
                let mut best: Option<((usize, u32), (f32, f32, Option<f32>))> = None;
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
        run(&mut g, 1.5);
        for _ in 0..lvl.pigs.len() {
            // Weak shot straight down into the ground.
            g.launch(V2::new(-0.5, 0.5));
            run(&mut g, 15.0);
        }
        assert!(matches!(g.phase, Phase::Lost { .. } | Phase::Won { .. }));
    }
}
