//! Grumpy Pigs — fling furious pigs at the smug wolves who stole their mud.

mod levels;
mod render;
mod sim;

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

/// Draws a rounded-ish button and reports whether it was tapped this frame.
fn button(rect: Rect, label: &str, input: &Input, accent: Color) -> bool {
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

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a,
    )
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
        render::pig(
            &deco,
            sim::PigKind::Grunt,
            V2::new(8.0, 0.9 + (self.time * 4.0).sin().abs() * 0.6),
            (self.time * 2.0).sin() * 0.2,
            self.time,
        );
        render::pig(&deco, sim::PigKind::Hog, V2::new(4.5, 0.7), 0.0, self.time);

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
            for s in 0..3 {
                let c = vec2(
                    r.center().x + (s as f32 - 1.0) * bw * 0.3,
                    r.bottom() + label_size * 3.0,
                );
                render::star(c, bw * 0.12, (s as u8) < self.best_stars[i]);
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
        let Screen::Play { idx, game } = &mut self.screen else {
            return;
        };
        let idx = *idx;
        let u = ui_unit();
        let (sw, sh) = (screen_width(), screen_height());

        // --- HUD buttons (consume taps before gameplay sees them) ---
        let bs = u * 0.09;
        let menu_r = Rect::new(u * 0.03, u * 0.03, bs, bs);
        let retry_r = Rect::new(u * 0.03 + bs * 1.2, u * 0.03, bs, bs);
        let ended = matches!(game.phase, Phase::Won { .. } | Phase::Lost { .. });

        let mut go_menu = false;
        let mut restart = false;
        let mut next = false;
        let on_hud = input
            .pressed
            .is_some_and(|p| menu_r.contains(p) || retry_r.contains(p));

        // --- Gameplay input ---
        if !ended && !on_hud {
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
                Phase::Flying { .. } if input.pressed.is_some() => {
                    game.ability();
                }
                _ => {}
            }
        }

        game.update(dt);

        // --- Effects from simulation events ---
        for ev in game.events.drain(..) {
            match ev {
                Event::Destroyed { pos, kind, points } => {
                    let color = match kind {
                        Kind::Block(Material::Straw) => Color::from_rgba(236, 205, 110, 255),
                        Kind::Block(Material::Wood) => Color::from_rgba(170, 110, 60, 255),
                        Kind::Block(Material::Brick) => Color::from_rgba(178, 82, 64, 255),
                        _ => Color::from_rgba(150, 150, 165, 255),
                    };
                    let n = if kind == Kind::Wolf { 22 } else { 12 };
                    for _ in 0..n {
                        self.particles.push(Particle {
                            pos: vec2(pos.x, pos.y),
                            vel: vec2(rand::gen_range(-5.0, 5.0), rand::gen_range(0.0, 7.0)),
                            life: rand::gen_range(0.5, 1.1),
                            max_life: 1.1,
                            size: rand::gen_range(0.08, 0.22),
                            color,
                            gravity: true,
                        });
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
                    for _ in 0..50 {
                        let a = rand::gen_range(0.0, std::f32::consts::TAU);
                        let sp = rand::gen_range(2.0, radius * 3.0);
                        self.particles.push(Particle {
                            pos: vec2(pos.x, pos.y),
                            vel: vec2(a.cos(), a.sin()) * sp,
                            life: rand::gen_range(0.3, 0.8),
                            max_life: 0.8,
                            size: rand::gen_range(0.2, 0.5),
                            color: if rand::gen_range(0, 2) == 0 {
                                Color::from_rgba(255, 190, 60, 255)
                            } else {
                                Color::from_rgba(255, 120, 40, 255)
                            },
                            gravity: false,
                        });
                    }
                }
                Event::Hit { pos, strength } => {
                    for _ in 0..(strength as usize).min(8) {
                        self.particles.push(Particle {
                            pos: vec2(pos.x, pos.y),
                            vel: vec2(rand::gen_range(-3.0, 3.0), rand::gen_range(0.0, 4.0)),
                            life: 0.4,
                            max_life: 0.4,
                            size: 0.08,
                            color: Color::new(1.0, 1.0, 0.9, 1.0),
                            gravity: true,
                        });
                    }
                }
                Event::Dash { pos } => {
                    for _ in 0..20 {
                        self.particles.push(Particle {
                            pos: vec2(pos.x, pos.y),
                            vel: vec2(rand::gen_range(-4.0, 1.0), rand::gen_range(-2.0, 2.0)),
                            life: 0.5,
                            max_life: 0.5,
                            size: rand::gen_range(0.1, 0.25),
                            color: Color::new(1.0, 0.85, 0.4, 1.0),
                            gravity: false,
                        });
                    }
                }
                Event::Poof { pos } => {
                    for _ in 0..14 {
                        self.particles.push(Particle {
                            pos: vec2(pos.x, pos.y),
                            vel: vec2(rand::gen_range(-2.0, 2.0), rand::gen_range(-1.0, 2.5)),
                            life: 0.7,
                            max_life: 0.7,
                            size: rand::gen_range(0.2, 0.4),
                            color: Color::new(1.0, 1.0, 1.0, 0.8),
                            gravity: false,
                        });
                    }
                }
                Event::Launch => {}
            }
        }

        // --- Draw world ---
        let aim_pos = game.aim.map(|p| Game::pouch() + p);
        let loaded = game.current_pig();
        let band_pos = loaded.map(|_| aim_pos.unwrap_or(Game::pouch()));
        render::sling_back(view, band_pos);
        render::queue(view, game, self.time);
        for e in game.ents.iter().filter(|e| e.alive) {
            match e.kind {
                Kind::Block(_) | Kind::Rock => render::block(view, e),
                Kind::Wolf => render::wolf(view, e),
                Kind::Pig(k) => render::pig(view, k, e.pos, e.angle, self.time),
            }
        }
        if let Some(k) = loaded {
            let p = band_pos.unwrap();
            render::pig(view, k, p, 0.0, self.time);
        }
        render::sling_front(view, band_pos);
        if let Some(pull) = game.aim
            && pull.length() > 0.5
        {
            render::trajectory(view, pull);
        }

        // Particles & popups
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
                Color::new(p.color.r, p.color.g, p.color.b, p.color.a * a),
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

        // --- HUD ---
        let level = &self.levels[idx];
        render::text_centered(
            &format!("{}. {}", idx + 1, level.name),
            sw / 2.0,
            u * 0.08,
            u * 0.055,
            WHITE,
        );
        let score = format!("{}", game.score);
        let sz = u * 0.07;
        let w = measure_text(&score, None, sz as u16, 1.0).width;
        draw_text(
            &score,
            sw - w - u * 0.04 + sz * 0.04,
            u * 0.1 + sz * 0.04,
            sz,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_text(&score, sw - w - u * 0.04, u * 0.1, sz, WHITE);
        if let Some((k, false)) = game.flying_pig() {
            let hint = k.hint();
            if !hint.is_empty() {
                render::text_centered(
                    hint,
                    sw / 2.0,
                    sh * 0.9,
                    u * 0.05,
                    Color::from_rgba(255, 230, 120, 255),
                );
            }
        }
        if game.phase == Phase::Aiming
            && game.aim.is_none()
            && game.time < 6.0
            && idx == 0
            && game.pigs_left.len() == level.pigs.len()
        {
            render::text_centered(
                "Drag back and release to fling!",
                sw / 2.0,
                sh * 0.9,
                u * 0.05,
                WHITE,
            );
        }

        go_menu |= button(menu_r, "=", input, SLATE);
        restart |= button(retry_r, "R", input, SLATE);

        // --- End-of-level overlay ---
        let mut stars_earned = None;
        match game.phase {
            Phase::Won { t } | Phase::Lost { t } if t > 0.6 => {
                let won = matches!(game.phase, Phase::Won { .. });
                let a = ((t - 0.6) * 3.0).min(1.0);
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45 * a));
                let pw = u * 0.9;
                let ph = u * 0.62;
                let panel = Rect::new((sw - pw) / 2.0, (sh - ph) / 2.0, pw, ph);
                draw_rectangle(
                    panel.x,
                    panel.y,
                    panel.w,
                    panel.h,
                    Color::new(0.98, 0.93, 0.85, a),
                );
                draw_rectangle_lines(
                    panel.x,
                    panel.y,
                    panel.w,
                    panel.h,
                    u * 0.012,
                    Color::new(0.55, 0.35, 0.2, a),
                );
                let title = if won {
                    "Wolves routed!"
                } else {
                    "The wolves are laughing..."
                };
                render::text_centered(
                    title,
                    sw / 2.0,
                    panel.y + ph * 0.17,
                    u * 0.07,
                    if won {
                        Color::from_rgba(90, 160, 70, 255)
                    } else {
                        Color::from_rgba(200, 80, 70, 255)
                    },
                );
                if won {
                    let st = game.stars();
                    stars_earned = Some(st);
                    for s in 0..3u8 {
                        let pop = ((t - 0.9 - s as f32 * 0.25) * 4.0).clamp(0.0, 1.0);
                        let c = vec2(sw / 2.0 + (s as f32 - 1.0) * u * 0.16, panel.y + ph * 0.38);
                        render::star(c, u * 0.065 * (0.6 + 0.4 * pop), s < st && pop > 0.0);
                    }
                }
                render::text_centered(
                    &format!("Score: {}", game.score),
                    sw / 2.0,
                    panel.y + ph * 0.6,
                    u * 0.055,
                    Color::from_rgba(80, 60, 40, 255),
                );
                let bh = u * 0.1;
                let bw = u * 0.24;
                let by = panel.bottom() - bh * 1.5;
                let has_next = won && idx + 1 < self.levels.len();
                let count = if has_next { 3.0 } else { 2.0 };
                let gap = u * 0.04;
                let x0 = sw / 2.0 - (count * bw + (count - 1.0) * gap) / 2.0;
                go_menu |= button(Rect::new(x0, by, bw, bh), "Menu", input, SLATE);
                restart |= button(Rect::new(x0 + bw + gap, by, bw, bh), "Retry", input, ORANGE);
                if has_next {
                    next |= button(
                        Rect::new(x0 + 2.0 * (bw + gap), by, bw, bh),
                        "Next",
                        input,
                        GREEN,
                    );
                }
            }
            _ => {}
        }

        if let Some(st) = stars_earned {
            self.best_stars[idx] = self.best_stars[idx].max(st);
        }
        if go_menu {
            self.screen = Screen::Menu;
        } else if restart {
            self.start(idx);
        } else if next {
            self.start(idx + 1);
        }
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

#[macroquad::main(conf)]
async fn main() {
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
            break;
        }
        next_frame().await;
    }
}
