//! The title screen: built-in levels, the player's level bank (tabs), and the
//! card shown when someone opens a shared level link.

use macroquad::prelude::*;

use super::{
    App, GREEN, Icon, Input, ORANGE, SLATE, button, icon_button, menu_corner_controls, ui_unit,
};
use crate::art::{self, Anim, Mood};
use crate::bank::Shelf;
use crate::gfx::rgb;
use crate::render::{self, View};
use crate::share::{self, DecodeError};
use crate::sim::{self, Level, PieceKind, V2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Levels,
    Mine,
    Friends,
}

/// A level that arrived through a link, waiting for the player to decide.
pub struct Incoming {
    pub level: Result<Level, DecodeError>,
    pub saved: bool,
}

/// What a tap on the title screen asks for.
enum Action {
    None,
    PlayBuiltin(usize),
    Play(Level),
    /// Open in the editor: my saved level (with its id) or a remix (`None`).
    Edit(Level, Option<String>),
    NewLevel,
    Save(Level),
    /// Delete a bank entry, by id.
    Delete(String),
}

const RED: Color = Color::new(0.78, 0.3, 0.27, 1.0);

/// The text a Share button shares for a level.
pub fn share_link(level: &Level) -> String {
    format!("{}#{}", crate::platform::site_url(), share::encode(level))
}

pub fn wolf_count(level: &Level) -> usize {
    level
        .pieces
        .iter()
        .filter(|p| matches!(p.kind, PieceKind::Wolf(_)))
        .count()
}

fn counts(level: &Level) -> String {
    let wolves = wolf_count(level);
    let pigs = level.pigs.len();
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            one.to_owned()
        } else {
            many.to_owned()
        }
    };
    format!(
        "{wolves} {}  -  {pigs} {}",
        plural(wolves, "wolf", "wolves"),
        plural(pigs, "pig", "pigs")
    )
}

/// Truncates to fit roughly `max` characters, for names of any length.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

fn panel(rect: Rect, fill: Color) {
    let r = rect.h.min(rect.w) * 0.08;
    draw_rectangle(rect.x + r, rect.y, rect.w - 2.0 * r, rect.h, fill);
    draw_rectangle(rect.x, rect.y + r, rect.w, rect.h - 2.0 * r, fill);
    for (cx, cy) in [
        (rect.x + r, rect.y + r),
        (rect.right() - r, rect.y + r),
        (rect.x + r, rect.bottom() - r),
        (rect.right() - r, rect.bottom() - r),
    ] {
        draw_circle(cx, cy, r, fill);
    }
}

/// A Share button for `level`; shows a toast with the outcome later.
fn share_button(rect: Rect, level: &Level, input: &Input) {
    let link = share_link(level);
    crate::platform::share_hotspot(rect, &link);
    if icon_button(rect, Icon::Share, input, SLATE) {
        crate::platform::activate(crate::platform::Hotspot::Share);
    }
}

impl App {
    pub(super) fn menu(&mut self, input: &Input) {
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.15));
        self.mascots();

        render::text_centered(
            "GRUMPY PIGS",
            sw / 2.0,
            sh * 0.15,
            u * 0.13,
            rgb(255, 170, 190),
        );
        render::text_centered(
            "Those wolves took our mud. Get it back.",
            sw / 2.0,
            sh * 0.215,
            u * 0.042,
            WHITE,
        );

        // While a shared-level card is up, it takes all the input.
        let blocked = Input::default();
        let ui_input = if self.incoming.is_some() {
            &blocked
        } else {
            input
        };

        if menu_corner_controls(ui_input, self.mirrored) {
            self.toggle_mirror();
        }
        let tab_action = self.tabs(ui_input);
        let action = match self.tab {
            Tab::Levels => self.builtin_grid(ui_input),
            Tab::Mine => self.bank_cards(Shelf::Mine, ui_input),
            Tab::Friends => self.bank_cards(Shelf::Friends, ui_input),
        };
        let action = match (action, tab_action) {
            (Action::None, Action::None) => self.incoming_card(input),
            (Action::None, a) | (a, _) => a,
        };
        self.apply(action);
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::PlayBuiltin(i) => self.start(i),
            Action::Play(level) => self.start_custom(level),
            Action::Edit(level, id) => self.edit_level(level, id),
            Action::NewLevel => self.new_level(),
            Action::Save(level) => {
                if self.bank.add(&level, Shelf::Friends).is_some() {
                    self.toast("Saved to From friends");
                }
            }
            Action::Delete(id) => {
                self.bank.remove(&id);
                self.confirm_delete = None;
                self.toast("Deleted");
            }
        }
    }

    fn mascots(&self) {
        let deco = View::fit(Vec2::ZERO, self.mirrored);
        let hop = (self.time * 4.0).sin().abs();
        let mut grunt = Anim::new(self.time, 3, Mood::Yell, V2::new(1.0, 0.3));
        grunt.stretch = (hop - 0.5) * 0.2;
        let tilt = (self.time * 2.0).sin() * 0.2;
        art::pig(
            &deco,
            sim::PigKind::Grunt,
            V2::new(8.0, 0.5 + hop * 0.8),
            tilt,
            &grunt,
        );
        let hog = Anim::new(self.time, 5, Mood::Idle, V2::new(1.0, 0.2));
        art::pig(&deco, sim::PigKind::Hog, V2::new(4.5, 0.7), 0.0, &hog);
        let smug = Anim::new(self.time, 9, Mood::Idle, V2::new(-1.0, -0.2));
        let boss = sim::WolfKind::Boss;
        art::wolf(&deco, boss, V2::new(35.0, 0.95), 0.0, 0.95, 1.0, 0.0, &smug);
    }

    /// The tab row, plus the current tab's action button (New / Paste link).
    fn tabs(&mut self, input: &Input) -> Action {
        let u = ui_unit();
        let sw = screen_width();
        let mine = self.bank.on_shelf(Shelf::Mine).count();
        let friends = self.bank.on_shelf(Shelf::Friends).count();
        let tabs = [
            (Tab::Levels, "Levels".to_owned()),
            (Tab::Mine, format!("My levels ({mine})")),
            (Tab::Friends, format!("From friends ({friends})")),
        ];
        let (w, h, gap) = (u * 0.34, u * 0.075, u * 0.02);
        let action_w = u * 0.24;
        let x0 = (sw - (w * 3.0 + gap * 3.0 + action_w)) / 2.0;
        let y = screen_height() * 0.255;
        for (i, (tab, label)) in tabs.into_iter().enumerate() {
            let r = Rect::new(x0 + i as f32 * (w + gap), y, w, h);
            let active = self.tab == tab;
            let color = if active {
                ORANGE
            } else {
                Color::new(0.0, 0.0, 0.0, 0.25)
            };
            if button(r, &label, input, color) && !active {
                self.tab = tab;
                self.page = 0;
                self.confirm_delete = None;
            }
        }
        let r = Rect::new(x0 + 3.0 * (w + gap), y, action_w, h);
        match self.tab {
            Tab::Levels => {}
            Tab::Mine => {
                if button(r, "+ New", input, GREEN) {
                    return Action::NewLevel;
                }
            }
            Tab::Friends => {
                crate::platform::prompt_hotspot(
                    r,
                    crate::platform::Ask::PasteLink,
                    "Paste a level link",
                    "",
                );
                if button(r, "Paste link", input, GREEN) {
                    // The web opens a prompt from the hotspot instead.
                    #[cfg(not(target_arch = "wasm32"))]
                    self.paste_link_natively();
                }
            }
        }
        Action::None
    }

    /// Desktop builds read a pasted link straight from the clipboard.
    #[cfg(not(target_arch = "wasm32"))]
    fn paste_link_natively(&mut self) {
        let text = crate::platform::clipboard().unwrap_or_default();
        self.incoming = Some(Incoming {
            level: share::decode(&text),
            saved: false,
        });
    }

    fn builtin_grid(&self, input: &Input) -> Action {
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        let n = self.levels.len();
        let cols = n.div_ceil(2).max(1);
        let bw = (u * 0.13).min(sw * 0.8 / cols as f32 - u * 0.03);
        let gap = u * 0.035;
        let pitch_y = bw + u * 0.095;
        let total = cols as f32 * bw + (cols - 1) as f32 * gap;
        let x0 = (sw - total) / 2.0;
        let y0 = sh * 0.36;
        let mut action = Action::None;
        for i in 0..n {
            let (row, col) = (i / cols, i % cols);
            let r = Rect::new(
                x0 + col as f32 * (bw + gap),
                y0 + row as f32 * pitch_y,
                bw,
                bw,
            );
            if button(r, &format!("{}", i + 1), input, ORANGE) {
                action = Action::PlayBuiltin(i);
            }
            let label_size = u * 0.022;
            let name = fit(&self.levels[i].name, 14);
            render::text_centered(
                &name,
                r.center().x,
                r.bottom() + label_size * 1.4,
                label_size,
                WHITE,
            );
            for s in 0..3u8 {
                let c = vec2(
                    r.center().x + (f32::from(s) - 1.0) * bw * 0.3,
                    r.bottom() + label_size * 2.6,
                );
                render::star(c, bw * 0.1, s < self.best_stars[i]);
            }
        }
        render::text_centered(
            "Pull back anywhere to aim  -  release to fling  -  tap mid-air for powers",
            sw / 2.0,
            sh * 0.855,
            u * 0.032,
            WHITE,
        );
        action
    }

    #[allow(clippy::too_many_lines)]
    fn bank_cards(&mut self, place: Shelf, input: &Input) -> Action {
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        let entries: Vec<(String, String, Result<Level, DecodeError>)> = self
            .bank
            .on_shelf(place)
            .map(|e| (e.id.clone(), e.code.clone(), e.level.clone()))
            .collect();

        if entries.is_empty() {
            let lines: &[&str] = match place {
                Shelf::Mine => &[
                    "Tap + New to build a level,",
                    "or tap the pencil while playing any level to remix it.",
                ],
                Shelf::Friends => &[
                    "Open a level link a friend sent you and tap Save,",
                    "or copy the link and tap Paste link.",
                ],
            };
            for (i, line) in lines.iter().enumerate() {
                let y = sh * 0.5 + i as f32 * u * 0.06;
                render::text_centered(line, sw / 2.0, y, u * 0.038, WHITE);
            }
            return Action::None;
        }

        let (cols, rows) = (3usize, 2usize);
        let per_page = cols * rows;
        let pages = entries.len().div_ceil(per_page);
        self.page = self.page.min(pages - 1);
        let (cw, ch, gap) = (u * 0.5, u * 0.24, u * 0.03);
        let cw = cw.min((sw - u * 0.3 - gap * 2.0) / 3.0);
        let x0 = (sw - (cw * 3.0 + gap * 2.0)) / 2.0;
        let y0 = sh * 0.36;
        let mut action = Action::None;

        for (slot, (id, code, level)) in entries
            .iter()
            .skip(self.page * per_page)
            .take(per_page)
            .enumerate()
        {
            let (row, col) = (slot / cols, slot % cols);
            let card = Rect::new(
                x0 + col as f32 * (cw + gap),
                y0 + row as f32 * (ch + gap),
                cw,
                ch,
            );
            panel(card, Color::new(0.98, 0.94, 0.86, 1.0));
            let pad = u * 0.02;
            let ts = u * 0.036;
            match level {
                Ok(level) => {
                    let name = if level.name.is_empty() {
                        "Untitled"
                    } else {
                        &level.name
                    };
                    draw_text(
                        fit(name, 18),
                        card.x + pad,
                        card.y + pad + ts,
                        ts,
                        rgb(70, 45, 30),
                    );
                    let by = if level.author.is_empty() {
                        "by someone".to_owned()
                    } else {
                        format!("by {}", fit(&level.author, 16))
                    };
                    let small = ts * 0.75;
                    draw_text(
                        &by,
                        card.x + pad,
                        card.y + pad + ts + small * 1.4,
                        small,
                        rgb(120, 95, 70),
                    );
                    draw_text(
                        counts(level),
                        card.x + pad,
                        card.y + pad + ts + small * 2.7,
                        small,
                        rgb(120, 95, 70),
                    );
                    let stars = self.custom_stars.get(code).copied().unwrap_or(0);
                    for s in 0..3u8 {
                        let c = vec2(
                            card.right() - pad - (2.5 - f32::from(s)) * ts * 0.9,
                            card.y + pad + ts * 0.55,
                        );
                        render::star(c, ts * 0.4, s < stars);
                    }
                }
                Err(e) => {
                    draw_text(
                        "Can't open this level",
                        card.x + pad,
                        card.y + pad + ts,
                        ts,
                        rgb(70, 45, 30),
                    );
                    draw_text(
                        e.message(),
                        card.x + pad,
                        card.y + pad + ts * 2.0,
                        ts * 0.6,
                        rgb(120, 95, 70),
                    );
                }
            }

            // Buttons along the bottom of the card.
            let bh = ch * 0.28;
            let by = card.bottom() - pad - bh;
            let confirming = self.confirm_delete.as_deref() == Some(id.as_str());
            let del = Rect::new(card.right() - pad - bh, by, bh, bh);
            if confirming {
                let sure = Rect::new(card.right() - pad - bh * 3.2, by, bh * 3.2, bh);
                if button(sure, "Delete?", input, RED) {
                    action = Action::Delete(id.clone());
                }
            } else if icon_button(del, Icon::Trash, input, RED) {
                self.confirm_delete = Some(id.clone());
            }
            if let Ok(level) = level {
                let play = Rect::new(card.x + pad, by, bh * 2.6, bh);
                if button(play, "Play", input, ORANGE) {
                    action = Action::Play(level.clone());
                }
                let share_r = Rect::new(play.right() + pad * 0.6, by, bh, bh);
                share_button(share_r, level, input);
                let edit_r = Rect::new(share_r.right() + pad * 0.6, by, bh, bh);
                if icon_button(edit_r, Icon::Pencil, input, SLATE) {
                    let mine = (place == Shelf::Mine).then(|| id.clone());
                    action = Action::Edit(level.clone(), mine);
                }
            }
        }

        if pages > 1 {
            let bs = u * 0.08;
            let y = y0 + ch + gap / 2.0 - bs / 2.0;
            let label = format!("{} / {}", self.page + 1, pages);
            render::text_centered(
                &label,
                sw / 2.0,
                y0 + 2.0 * ch + gap + u * 0.055,
                u * 0.035,
                WHITE,
            );
            if self.page > 0 && button(Rect::new(x0 - bs - gap, y, bs, bs), "<", input, SLATE) {
                self.page -= 1;
            }
            let right = Rect::new(x0 + cw * 3.0 + gap * 3.0, y, bs, bs);
            if self.page + 1 < pages && button(right, ">", input, SLATE) {
                self.page += 1;
            }
        }
        action
    }

    /// The "a friend sent you a level" card, if one is waiting.
    fn incoming_card(&mut self, input: &Input) -> Action {
        let Some(incoming) = &self.incoming else {
            return Action::None;
        };
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
        let (pw, ph) = (u * 0.95, u * 0.6);
        let card = Rect::new((sw - pw) / 2.0, (sh - ph) / 2.0, pw, ph);
        panel(card, Color::new(0.98, 0.94, 0.86, 1.0));
        let (bw, bh, gap) = (u * 0.26, u * 0.1, u * 0.03);
        let by = card.bottom() - bh * 1.5;

        let mut action = Action::None;
        let mut close = false;
        match &incoming.level {
            Ok(level) => {
                render::text_centered(
                    "Someone sent you a level!",
                    sw / 2.0,
                    card.y + ph * 0.16,
                    u * 0.05,
                    rgb(90, 160, 70),
                );
                let name = if level.name.is_empty() {
                    "Untitled"
                } else {
                    &level.name
                };
                render::text_centered(
                    &fit(name, 26),
                    sw / 2.0,
                    card.y + ph * 0.36,
                    u * 0.07,
                    rgb(70, 45, 30),
                );
                let by_line = if level.author.is_empty() {
                    counts(level)
                } else {
                    format!("by {}   -   {}", fit(&level.author, 20), counts(level))
                };
                render::text_centered(
                    &by_line,
                    sw / 2.0,
                    card.y + ph * 0.5,
                    u * 0.038,
                    rgb(120, 95, 70),
                );

                let x0 = (sw - (bw * 3.0 + gap * 2.0)) * 0.5;
                let slot = |i: f32| Rect::new(x0 + i * (bw + gap), by, bw, bh);
                if button(slot(0.0), "Close", input, SLATE) {
                    close = true;
                }
                let saved = incoming.saved || self.bank.contains(&share::encode(level));
                if saved {
                    button(slot(1.0), "Saved", input, Color::new(0.5, 0.6, 0.5, 1.0));
                } else if button(slot(1.0), "Save", input, GREEN) {
                    action = Action::Save(level.clone());
                }
                if button(slot(2.0), "Play", input, ORANGE) {
                    action = Action::Play(level.clone());
                    close = true;
                }
            }
            Err(e) => {
                render::text_centered(
                    "Couldn't open that level",
                    sw / 2.0,
                    card.y + ph * 0.25,
                    u * 0.055,
                    rgb(200, 80, 70),
                );
                render::text_centered(
                    e.message(),
                    sw / 2.0,
                    card.y + ph * 0.45,
                    u * 0.04,
                    rgb(120, 95, 70),
                );
                let r = Rect::new(sw / 2.0 - bw / 2.0, by, bw, bh);
                if button(r, "OK", input, SLATE) {
                    close = true;
                }
            }
        }
        if let (Action::Save(_), Some(inc)) = (&action, self.incoming.as_mut()) {
            inc.saved = true;
        }
        if close {
            self.incoming = None;
        }
        action
    }
}
