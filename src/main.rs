//! Grumpy Pigs — fling furious pigs at the smug wolves who stole their mud.

mod art;
mod gfx;
mod levels;
mod platform;
mod render;
mod sim;

use art::{Anim, Mood};
use gfx::{mix, rgb};
use macroquad::prelude::*;
use render::View;
use sim::{Event, Game, Kind, Level, Material, Phase, V2};

fn conf() -> Conf {
    Conf {
        window_title: "Grumpy Pigs".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

struct Particle {
    pos: Vec2, // world
    vel: Vec2,
    life: f32,
    max_life: f32,
    size: f32,
    color: Color,
    gravity: bool,
}

struct Popup {
    pos: Vec2,
    text: String,
    life: f32,
}

enum Screen {
    Menu,
    Play { idx: usize, game: Box<Game> },
}

struct App {
    levels: Vec<Level>,
    best_stars: Vec<u8>,
    screen: Screen,
    particles: Vec<Particle>,
    popups: Vec<Popup>,
    shake: f32,
    /// Wolves mock a missed shot for a moment.
    laugh: f32,
    trail_t: f32,
    aim_start: Option<Vec2>,
    time: f32,
}

#[derive(Default)]
struct Input {
    pressed: Option<Vec2>,
    down: Option<Vec2>,
    released: bool,
}

impl Input {
    fn read() -> Self {
        let pos: Vec2 = mouse_position().into();
        Input {
            pressed: is_mouse_button_pressed(MouseButton::Left).then_some(pos),
            down: is_mouse_button_down(MouseButton::Left).then_some(pos),
            released: is_mouse_button_released(MouseButton::Left),
        }
    }
}

fn ui_unit() -> f32 {
    screen_height().min(screen_width() * 0.6)
}

fn button_bg(rect: Rect, accent: Color) {
    let hover = rect.contains(mouse_position().into());
    let col = if hover {
        mix(accent, WHITE, 0.15)
    } else {
        accent
    };
    let r = rect.h * 0.25;
    draw_rectangle(rect.x + r, rect.y, rect.w - 2.0 * r, rect.h, col);
    draw_rectangle(rect.x, rect.y + r, rect.w, rect.h - 2.0 * r, col);
    for (cx, cy) in [
        (rect.x + r, rect.y + r),
        (rect.right() - r, rect.y + r),
        (rect.x + r, rect.bottom() - r),
        (rect.right() - r, rect.bottom() - r),
    ] {
        draw_circle(cx, cy, r, col);
    }
}

/// Draws a rounded button with a text label; true if tapped this frame.
fn button(rect: Rect, label: &str, input: &Input, accent: Color) -> bool {
    button_bg(rect, accent);
    let size = rect.h * 0.55;
    render::text_centered(
        label,
        rect.center().x,
        rect.center().y + size * 0.35,
        size,
        WHITE,
    );
    input.pressed.is_some_and(|p| rect.contains(p))
}

#[derive(Clone, Copy)]
enum Icon {
    Menu,
    Restart,
    Expand,
    Collapse,
}

/// Draws a rounded button with a line icon; true if tapped this frame.
fn icon_button(rect: Rect, icon: Icon, input: &Input, accent: Color) -> bool {
    button_bg(rect, accent);
    let c = rect.center();
    let k = rect.h * 0.24; // icon half-size
    let w = rect.h * 0.08; // stroke width
    let line = |a: Vec2, b: Vec2| draw_line(a.x, a.y, b.x, b.y, w, WHITE);
    match icon {
        Icon::Menu => {
            for dy in [-0.7, 0.0, 0.7] {
                line(c + vec2(-k, dy * k), c + vec2(k, dy * k));
            }
        }
        Icon::Restart => {
            // Three-quarter circle with an arrow head at its open end.
            let pts: Vec<Vec2> = (0..=18)
                .map(|i| {
                    let a = -0.9 + i as f32 / 18.0 * 4.9;
                    c + vec2(a.cos(), a.sin()) * k * 0.9
                })
                .collect();
            for seg in pts.windows(2) {
                line(seg[0], seg[1]);
            }
            for p in &pts {
                draw_circle(p.x, p.y, w / 2.0, WHITE);
            }
            let tip = pts[pts.len() - 1];
            draw_triangle(
                tip + vec2(-k * 0.45, -k * 0.05),
                tip + vec2(k * 0.35, k * 0.3),
                tip + vec2(k * 0.1, -k * 0.55),
                WHITE,
            );
        }
        Icon::Expand | Icon::Collapse => {
            let inward = matches!(icon, Icon::Collapse);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = c + vec2(sx, sy) * k;
                let arm = k * 0.55;
                // Expand: brackets open outward at the corners. Collapse: point inward.
                let (hx, vy) = if inward {
                    (sx * arm, sy * arm)
                } else {
                    (-sx * arm, -sy * arm)
                };
                let pivot = if inward {
                    corner - vec2(sx, sy) * arm
                } else {
                    corner
                };
                line(pivot, pivot + vec2(hx, 0.0));
                line(pivot, pivot + vec2(0.0, vy));
                draw_circle(pivot.x, pivot.y, w / 2.0, WHITE);
            }
        }
    }
    input.pressed.is_some_and(|p| rect.contains(p))
}

/// The fullscreen toggle, when the platform supports it. True if tapped.
fn fullscreen_button(rect: Rect, input: &Input) -> bool {
    let icon = match platform::fullscreen() {
        platform::Fullscreen::Unavailable => return false,
        platform::Fullscreen::Windowed => Icon::Expand,
        platform::Fullscreen::Full => Icon::Collapse,
    };
    platform::hotspot(platform::Hotspot::Fullscreen, rect);
    let tapped = icon_button(rect, icon, input, SLATE);
    if tapped {
        platform::activate(platform::Hotspot::Fullscreen);
    }
    tapped
}

const GREEN: Color = Color::new(0.35, 0.68, 0.3, 1.0);
const ORANGE: Color = Color::new(0.93, 0.55, 0.2, 1.0);
const SLATE: Color = Color::new(0.35, 0.38, 0.48, 1.0);

impl App {
    fn new() -> Self {
        let levels = levels::all();
        App {
            best_stars: vec![0; levels.len()],
            levels,
            screen: Screen::Menu,
            particles: Vec::new(),
            popups: Vec::new(),
            shake: 0.0,
            laugh: 0.0,
            trail_t: 0.0,
            aim_start: None,
            time: 0.0,
        }
    }

    fn start(&mut self, idx: usize) {
        self.particles.clear();
        self.popups.clear();
        self.aim_start = None;
        self.screen = Screen::Play {
            idx,
            game: Box::new(Game::new(&self.levels[idx])),
        };
    }

    fn frame(&mut self) {
        let dt = get_frame_time().min(0.05);
        self.time += dt;
        platform::begin_frame();
        let input = Input::read();

        let shake = if self.shake > 0.0 {
            let m = self.shake * ui_unit() * 0.02;
            vec2(rand::gen_range(-m, m), rand::gen_range(-m, m))
        } else {
            Vec2::ZERO
        };
        self.shake = (self.shake - dt * 3.0).max(0.0);
        let view = View::fit(shake);
        render::background(&view, self.time);

        match &self.screen {
            Screen::Menu => self.menu(&input),
            Screen::Play { .. } => self.play(&input, &view, dt),
        }
    }

    fn menu(&mut self, input: &Input) {
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.15));

        // A couple of mascots
        let deco = View::fit(Vec2::ZERO);
        let hop = (self.time * 4.0).sin().abs();
        let mut grunt = Anim::new(self.time, 3, Mood::Yell, V2::new(1.0, 0.3));
        grunt.stretch = (hop - 0.5) * 0.2;
        art::pig(
            &deco,
            sim::PigKind::Grunt,
            V2::new(8.0, 0.5 + hop * 0.8),
            (self.time * 2.0).sin() * 0.2,
            &grunt,
        );
        let hog = Anim::new(self.time, 5, Mood::Idle, V2::new(1.0, 0.2));
        art::pig(&deco, sim::PigKind::Hog, V2::new(4.5, 0.7), 0.0, &hog);
        let smug = Anim::new(self.time, 9, Mood::Idle, V2::new(-1.0, -0.2));
        art::wolf(&deco, V2::new(35.0, 0.7), 0.0, 0.7, 1.0, 0.0, &smug);

        render::text_centered(
            "GRUMPY PIGS",
            sw / 2.0,
            sh * 0.22,
            u * 0.14,
            Color::from_rgba(255, 170, 190, 255),
        );
        render::text_centered(
            "Those wolves took our mud. Get it back.",
            sw / 2.0,
            sh * 0.30,
            u * 0.045,
            WHITE,
        );

        let bs = u * 0.09;
        let full = Rect::new(sw - u * 0.03 - bs, u * 0.03, bs, bs);
        fullscreen_button(full, input);
        match platform::install() {
            platform::Install::Prompt => {
                let r = Rect::new(full.x - bs * 0.3 - bs * 2.4, full.y, bs * 2.4, bs);
                platform::hotspot(platform::Hotspot::Install, r);
                if button(r, "Install", input, GREEN) {
                    platform::activate(platform::Hotspot::Install);
                }
            }
            platform::Install::ShareSheet => {
                render::text_centered(
                    "Play it like an app: tap Share, then Add to Home Screen",
                    sw / 2.0,
                    sh * 0.89,
                    u * 0.032,
                    WHITE,
                );
            }
            platform::Install::Unavailable => {}
        }

        let n = self.levels.len();
        let bw = u * 0.22;
        let gap = u * 0.04;
        let total = n as f32 * bw + (n - 1) as f32 * gap;
        let x0 = (sw - total) / 2.0;
        let y = sh * 0.42;
        let mut chosen = None;
        for i in 0..n {
            let r = Rect::new(x0 + i as f32 * (bw + gap), y, bw, bw);
            if button(r, &format!("{}", i + 1), input, ORANGE) {
                chosen = Some(i);
            }
            let label_size = u * 0.03;
            render::text_centered(
                self.levels[i].name,
                r.center().x,
                r.bottom() + label_size * 1.4,
                label_size,
                WHITE,
            );
            for s in 0..3u8 {
                let c = vec2(
                    r.center().x + (f32::from(s) - 1.0) * bw * 0.3,
                    r.bottom() + label_size * 3.0,
                );
                render::star(c, bw * 0.12, s < self.best_stars[i]);
            }
        }
        render::text_centered(
            "Pull back anywhere to aim  -  release to fling  -  tap mid-air for powers",
            sw / 2.0,
            sh * 0.82,
            u * 0.035,
            WHITE,
        );
        if let Some(i) = chosen {
            self.start(i);
        }
    }

    fn play(&mut self, input: &Input, view: &View, dt: f32) {
        let Screen::Play { idx, mut game } = std::mem::replace(&mut self.screen, Screen::Menu)
        else {
            return;
        };
        let hud = HudButtons::new();
        let ended = matches!(game.phase, Phase::Won { .. } | Phase::Lost { .. });
        if !ended && !input.pressed.is_some_and(|p| hud.contains(p)) {
            self.handle_input(&mut game, input, view);
        }

        game.update(dt);
        self.apply_events(&mut game);
        self.animate(&game, dt);
        self.draw_world(&game, view);
        self.draw_effects(view, dt);

        let mut nav = self.draw_hud(idx, &game, input, &hud);
        let overlay = self.end_overlay(idx, &game, input);
        if nav == Nav::Stay {
            nav = overlay;
        }

        self.screen = Screen::Play { idx, game };
        match nav {
            Nav::Stay => {}
            Nav::Menu => self.screen = Screen::Menu,
            Nav::Restart => self.start(idx),
            Nav::Next => self.start(idx + 1),
        }
    }

    fn handle_input(&mut self, game: &mut Game, input: &Input, view: &View) {
        match game.phase {
            Phase::Aiming => {
                if let Some(p) = input.pressed {
                    self.aim_start = Some(p);
                }
                if let (Some(start), Some(cur)) = (self.aim_start, input.down) {
                    let d = (cur - start) / view.s;
                    let pull = V2::new(d.x, -d.y).clamp_length_max(sim::MAX_PULL);
                    game.aim = Some(pull);
                }
                if input.released {
                    if let Some(pull) = game.aim.take()
                        && pull.length() > 0.5
                    {
                        game.launch(pull);
                    }
                    self.aim_start = None;
                }
            }
            Phase::Flying { .. } if input.pressed.is_some() => game.ability(),
            _ => {}
        }
    }

    fn spawn(&mut self, pos: V2, vel: Vec2, life: f32, size: f32, color: Color, gravity: bool) {
        self.particles.push(Particle {
            pos: vec2(pos.x, pos.y),
            vel,
            life,
            max_life: life,
            size,
            color,
            gravity,
        });
    }

    /// Turns simulation events into particles, popups, shake and taunts.
    fn apply_events(&mut self, game: &mut Game) {
        let wolves_alive = game.wolves_alive();
        for ev in std::mem::take(&mut game.events) {
            match ev {
                Event::Destroyed { pos, kind, points } => {
                    let color = match kind {
                        Kind::Block(Material::Straw) => rgb(236, 205, 110),
                        Kind::Block(Material::Wood) => rgb(170, 110, 60),
                        Kind::Block(Material::Brick) => rgb(178, 82, 64),
                        _ => rgb(150, 150, 165),
                    };
                    let n = if kind == Kind::Wolf { 22 } else { 12 };
                    for _ in 0..n {
                        let vel = vec2(rand::gen_range(-5.0, 5.0), rand::gen_range(0.0, 7.0));
                        let life = rand::gen_range(0.5, 1.1);
                        self.spawn(pos, vel, life, rand::gen_range(0.08, 0.22), color, true);
                    }
                    if points > 0 {
                        self.popups.push(Popup {
                            pos: vec2(pos.x, pos.y + 0.6),
                            text: points.to_string(),
                            life: 1.2,
                        });
                    }
                    if kind == Kind::Wolf {
                        self.shake = self.shake.max(0.4);
                    }
                }
                Event::Explosion { pos, radius } => {
                    self.shake = 1.0;
                    for i in 0..50 {
                        let a = rand::gen_range(0.0, std::f32::consts::TAU);
                        let vel = vec2(a.cos(), a.sin()) * rand::gen_range(2.0, radius * 3.0);
                        let color = if i % 2 == 0 {
                            rgb(255, 190, 60)
                        } else {
                            rgb(255, 120, 40)
                        };
                        let life = rand::gen_range(0.3, 0.8);
                        self.spawn(pos, vel, life, rand::gen_range(0.2, 0.5), color, false);
                    }
                }
                Event::Hit { pos, strength } => {
                    for _ in 0..gfx::count(strength, 8) {
                        let vel = vec2(rand::gen_range(-3.0, 3.0), rand::gen_range(0.0, 4.0));
                        self.spawn(pos, vel, 0.4, 0.08, Color::new(1.0, 1.0, 0.9, 1.0), true);
                    }
                }
                Event::Dash { pos } => {
                    for _ in 0..20 {
                        let vel = vec2(rand::gen_range(-4.0, 1.0), rand::gen_range(-2.0, 2.0));
                        let size = rand::gen_range(0.1, 0.25);
                        self.spawn(pos, vel, 0.5, size, Color::new(1.0, 0.85, 0.4, 1.0), false);
                    }
                }
                Event::Poof { pos } => {
                    if wolves_alive > 0 {
                        self.laugh = 2.0;
                    }
                    for _ in 0..14 {
                        let vel = vec2(rand::gen_range(-2.0, 2.0), rand::gen_range(-1.0, 2.5));
                        let size = rand::gen_range(0.2, 0.4);
                        self.spawn(pos, vel, 0.7, size, Color::new(1.0, 1.0, 1.0, 0.8), false);
                    }
                }
                Event::Launch => {}
            }
        }
    }

    /// Per-frame cosmetic state: taunt timer and the flight smoke trail.
    fn animate(&mut self, game: &Game, dt: f32) {
        self.laugh = (self.laugh - dt).max(0.0);
        let Some(f) = game.flight() else { return };
        let e = &game.ents[f.ent];
        if !e.alive || f.since_impact.is_some() {
            return;
        }
        self.trail_t += dt;
        while self.trail_t > 0.035 {
            self.trail_t -= 0.035;
            let vel = vec2(rand::gen_range(-0.3, 0.3), rand::gen_range(-0.3, 0.3));
            let size = rand::gen_range(0.12, 0.2);
            self.spawn(e.pos, vel, 0.6, size, Color::new(1.0, 1.0, 1.0, 0.6), false);
        }
    }

    fn draw_world(&self, game: &Game, view: &View) {
        let flight = game.flight().filter(|f| game.ents[f.ent].alive);
        let pig_pos = flight.map(|f| game.ents[f.ent].pos);
        let lost = matches!(game.phase, Phase::Lost { .. });

        let aim_pos = game.aim.map(|p| Game::pouch() + p);
        let loaded = game.current_pig();
        let band_pos = loaded.map(|_| aim_pos.unwrap_or(Game::pouch()));
        render::grass(view, self.time);
        render::sling_back(view, band_pos);
        render::queue(view, game, self.time);

        for (i, e) in game.ents.iter().enumerate().filter(|(_, e)| e.alive) {
            let mut anim = Anim::new(self.time, e.seed, Mood::Idle, V2::X);
            if e.impact > 0.02 {
                anim.stretch = -e.impact * 0.35;
                anim.stretch_dir = e.impact_dir;
            }
            match e.kind {
                Kind::Block(_) | Kind::Rock => render::block(view, e),
                Kind::Wolf => {
                    let sim::Shape::Ball { r } = e.shape else {
                        continue;
                    };
                    let near = pig_pos.is_some_and(|p| (p - e.pos).length() < 9.0);
                    anim.mood = if e.hurt_flash > 0.35 {
                        Mood::Hurt
                    } else if self.laugh > 0.0 || lost {
                        Mood::Laugh
                    } else if near || e.hp_frac() < 0.6 {
                        Mood::Worried
                    } else {
                        Mood::Idle
                    };
                    anim.look = pig_pos.unwrap_or(Game::pouch()) - e.pos;
                    art::wolf(view, e.pos, e.angle, r, e.hp_frac(), e.hurt_flash, &anim);
                }
                Kind::Pig(k) => {
                    let mine = flight.filter(|f| f.ent == i);
                    let airborne = mine.is_some_and(|f| f.since_impact.is_none());
                    anim.mood = if airborne { Mood::Yell } else { Mood::Dizzy };
                    anim.look = if e.vel.length() > 0.5 { e.vel } else { V2::X };
                    if airborne && e.impact <= 0.02 {
                        anim.stretch = (e.vel.length() / 70.0).min(0.22);
                        anim.stretch_dir = e.vel;
                    }
                    if let Some(f) = mine
                        && k == sim::PigKind::Boomer
                        && !f.used
                    {
                        anim.glow = f.since_impact.map_or(0.0, |s| (s / 1.2).min(1.0));
                    }
                    art::pig(view, k, e.pos, e.angle, &anim);
                }
            }
        }

        if let (Some(k), Some(p)) = (loaded, band_pos) {
            let pull = game.aim.unwrap_or(V2::ZERO);
            let mood = if game.aim.is_some() {
                Mood::Aim
            } else {
                Mood::Idle
            };
            let mut anim = Anim::new(self.time, 1, mood, V2::new(1.0, 0.2));
            if pull.length() > 0.1 {
                anim.stretch = pull.length() / sim::MAX_PULL * 0.18;
                anim.stretch_dir = pull;
            }
            art::pig(view, k, p, 0.0, &anim);
        }
        render::sling_front(view, band_pos);
        if let Some(pull) = game.aim
            && pull.length() > 0.5
        {
            render::trajectory(view, pull, self.time);
        }
    }

    fn draw_effects(&mut self, view: &View, dt: f32) {
        self.particles.retain_mut(|p| {
            p.life -= dt;
            if p.gravity {
                p.vel.y += sim::GRAVITY * dt;
            } else {
                p.vel *= 1.0 - 3.0 * dt;
            }
            p.pos += p.vel * dt;
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            let q = view.p(p.pos.x, p.pos.y);
            draw_circle(
                q.x,
                q.y,
                p.size * view.s * (0.5 + a * 0.5),
                gfx::alpha(p.color, a),
            );
            p.life > 0.0
        });
        self.popups.retain_mut(|p| {
            p.life -= dt;
            p.pos.y += dt * 1.2;
            let q = view.p(p.pos.x, p.pos.y);
            let a = p.life.clamp(0.0, 1.0);
            render::text_centered(
                &p.text,
                q.x,
                q.y,
                view.s * 0.9,
                Color::new(1.0, 1.0, 1.0, a),
            );
            p.life > 0.0
        });
    }

    fn draw_hud(&self, idx: usize, game: &Game, input: &Input, hud: &HudButtons) -> Nav {
        let u = ui_unit();
        let (sw, sh) = (screen_width(), screen_height());
        let level = &self.levels[idx];
        let title = format!("{}. {}", idx + 1, level.name);
        render::text_centered(&title, sw / 2.0, u * 0.08, u * 0.055, WHITE);

        let score = game.score.to_string();
        let sz = u * 0.07;
        let w = measure_text(&score, None, render::font_size(sz), 1.0).width;
        let (x, y) = (sw - w - u * 0.04, u * 0.1);
        draw_text(
            &score,
            x + sz * 0.04,
            y + sz * 0.04,
            sz,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_text(&score, x, y, sz, WHITE);

        let hint = match game.flying_pig() {
            Some((k, false)) => k.hint(),
            _ if game.phase == Phase::Aiming
                && game.aim.is_none()
                && game.time < 6.0
                && idx == 0
                && game.pigs_left.len() == level.pigs.len() =>
            {
                "Drag back and release to fling!"
            }
            _ => "",
        };
        if !hint.is_empty() {
            render::text_centered(hint, sw / 2.0, sh * 0.9, u * 0.05, rgb(255, 236, 150));
        }

        fullscreen_button(hud.full, input);
        if icon_button(hud.menu, Icon::Menu, input, SLATE) {
            Nav::Menu
        } else if icon_button(hud.retry, Icon::Restart, input, SLATE) {
            Nav::Restart
        } else {
            Nav::Stay
        }
    }

    /// Win/lose panel with stars and navigation; records best stars.
    fn end_overlay(&mut self, idx: usize, game: &Game, input: &Input) -> Nav {
        let (Phase::Won { t } | Phase::Lost { t }) = game.phase else {
            return Nav::Stay;
        };
        if t <= 0.6 {
            return Nav::Stay;
        }
        let won = matches!(game.phase, Phase::Won { .. });
        let u = ui_unit();
        let (sw, sh) = (screen_width(), screen_height());
        let a = ((t - 0.6) * 3.0).min(1.0);
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45 * a));
        let (pw, ph) = (u * 0.9, u * 0.62);
        let panel = Rect::new((sw - pw) / 2.0, (sh - ph) / 2.0, pw, ph);
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::new(0.98, 0.93, 0.85, a),
        );
        let border = Color::new(0.55, 0.35, 0.2, a);
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, u * 0.012, border);

        let (title, color) = if won {
            ("Wolves routed!", rgb(90, 160, 70))
        } else {
            ("The wolves are laughing...", rgb(200, 80, 70))
        };
        render::text_centered(title, sw / 2.0, panel.y + ph * 0.17, u * 0.07, color);
        if won {
            let st = game.stars();
            self.best_stars[idx] = self.best_stars[idx].max(st);
            for s in 0..3u8 {
                let fs = f32::from(s);
                let pop = ((t - 0.9 - fs * 0.25) * 4.0).clamp(0.0, 1.0);
                let c = vec2(sw / 2.0 + (fs - 1.0) * u * 0.16, panel.y + ph * 0.38);
                render::star(c, u * 0.065 * (0.6 + 0.4 * pop), s < st && pop > 0.0);
            }
        }
        let score = format!("Score: {}", game.score);
        render::text_centered(
            &score,
            sw / 2.0,
            panel.y + ph * 0.6,
            u * 0.055,
            rgb(80, 60, 40),
        );

        let (bw, bh, gap) = (u * 0.24, u * 0.1, u * 0.04);
        let by = panel.bottom() - bh * 1.5;
        let has_next = won && idx + 1 < self.levels.len();
        let count = if has_next { 3.0 } else { 2.0 };
        let total = count * bw + (count - 1.0) * gap;
        let x0 = (sw - total) * 0.5;
        let slot = |i: f32| Rect::new(x0 + i * (bw + gap), by, bw, bh);
        if button(slot(0.0), "Menu", input, SLATE) {
            Nav::Menu
        } else if button(slot(1.0), "Retry", input, ORANGE) {
            Nav::Restart
        } else if has_next && button(slot(2.0), "Next", input, GREEN) {
            Nav::Next
        } else {
            Nav::Stay
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Stay,
    Menu,
    Restart,
    Next,
}

/// The always-visible menu / retry / fullscreen buttons in the top-left corner.
struct HudButtons {
    menu: Rect,
    retry: Rect,
    full: Rect,
}

impl HudButtons {
    fn new() -> Self {
        let u = ui_unit();
        let bs = u * 0.09;
        HudButtons {
            menu: Rect::new(u * 0.03, u * 0.03, bs, bs),
            retry: Rect::new(u * 0.03 + bs * 1.2, u * 0.03, bs, bs),
            full: Rect::new(u * 0.03 + bs * 2.4, u * 0.03, bs, bs),
        }
    }
    fn contains(&self, p: Vec2) -> bool {
        let full =
            platform::fullscreen() != platform::Fullscreen::Unavailable && self.full.contains(p);
        self.menu.contains(p) || self.retry.contains(p) || full
    }
}

/// Dev aid (native only): `GRUMPY_SHOT=out.png GRUMPY_LEVEL=3 GRUMPY_FRAMES=240
/// GRUMPY_LAUNCH=30,1.0 cargo run` renders a scene, saves a PNG and exits.
/// `GRUMPY_AIM=deg,pow` instead freezes the sling mid-pull.
struct Shot {
    path: String,
    frames: u32,
    launch: Option<V2>,
    aim: Option<V2>,
}

fn pull_from_env(var: &str) -> Option<V2> {
    let s = std::env::var(var).ok()?;
    let (deg, pow) = s.split_once(',')?;
    let a = deg.trim().parse::<f32>().ok()?.to_radians();
    let pow: f32 = pow.trim().parse().ok()?;
    Some(V2::new(-a.cos(), -a.sin()) * sim::MAX_PULL * pow)
}

/// Dev aid: `GRUMPY_ICON=out.png GRUMPY_ICON_SCALE=0.36` renders the app icon
/// (a grumpy pig on sky and grass) centered in a square, saves it and exits.
/// `make-icons.sh` crops and resizes the result into `web/icons/`.
async fn render_icon(path: &str, scale: f32) {
    for frame in 0..12 {
        clear_background(BLACK);
        let (sw, sh) = (screen_width(), screen_height());
        let side = sh;
        let x0 = (sw - side) / 2.0;
        for i in 0..40 {
            let t = i as f32 / 40.0;
            let col = mix(rgb(98, 170, 232), rgb(214, 238, 250), t);
            draw_rectangle(x0, side * t, side, side / 40.0 + 1.0, col);
        }
        draw_rectangle(x0, side * 0.8, side, side * 0.2, rgb(98, 172, 74));
        draw_rectangle(x0, side * 0.8, side, side * 0.025, rgb(130, 200, 96));
        let r_px = side * scale;
        let pig = sim::PigKind::Grunt;
        let s = r_px / pig.radius();
        let view = View {
            s,
            ox: x0 + side / 2.0,
            oy: side / 2.0 + r_px * 0.12,
        };
        let anim = Anim::new(0.2, 3, Mood::Idle, V2::new(0.0, -1.0));
        art::pig(&view, pig, V2::ZERO, 0.0, &anim);
        if frame == 11 {
            get_screen_data().export_png(path);
            // The exported image is in physical pixels.
            let k = screen_dpi_scale();
            println!("{} 0 {}", x0 * k, side * k);
        }
        next_frame().await;
    }
}

#[macroquad::main(conf)]
async fn main() {
    if !cfg!(target_arch = "wasm32")
        && let Ok(path) = std::env::var("GRUMPY_ICON")
    {
        let scale = std::env::var("GRUMPY_ICON_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.36);
        render_icon(&path, scale).await;
        return;
    }
    let mut app = App::new();
    let shot = if cfg!(target_arch = "wasm32") {
        None
    } else {
        std::env::var("GRUMPY_SHOT").ok().map(|path| Shot {
            path,
            frames: std::env::var("GRUMPY_FRAMES")
                .ok()
                .and_then(|f| f.parse().ok())
                .unwrap_or(90),
            launch: pull_from_env("GRUMPY_LAUNCH"),
            aim: pull_from_env("GRUMPY_AIM"),
        })
    };
    if let Some(level) = std::env::var("GRUMPY_LEVEL")
        .ok()
        .and_then(|l| l.parse::<usize>().ok())
    {
        app.start(level.saturating_sub(1).min(app.levels.len() - 1));
    }
    let mut frame = 0u32;
    loop {
        if let (Some(shot), Screen::Play { game, .. }) = (&shot, &mut app.screen) {
            if frame == 80
                && let Some(p) = shot.launch
            {
                game.launch(p);
            }
            if shot.aim.is_some() && game.phase == Phase::Aiming {
                game.aim = shot.aim;
            }
        }
        clear_background(BLACK);
        app.frame();
        frame += 1;
        if let Some(shot) = &shot
            && frame >= shot.frames
        {
            get_screen_data().export_png(&shot.path);
            if let Screen::Play { game, .. } = &app.screen {
                eprintln!(
                    "phase={:?} laugh={:.2} pigs_left={}",
                    game.phase,
                    app.laugh,
                    game.pigs_left.len()
                );
            }
            break;
        }
        next_frame().await;
    }
}
