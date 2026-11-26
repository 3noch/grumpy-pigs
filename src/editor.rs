//! The level editor: drag pieces from a palette onto the scene, select them to
//! rotate / resize / restyle, move the launcher, pick the pig lineup, then
//! test, save and share.
//!
//! Undo is a history of level codes (the same text that goes in share links).

use std::f32::consts::TAU;

use macroquad::prelude::*;

use super::{
    App, Icon, Input, ORANGE, SLATE, Screen, Source, button, button_bg, icon_button,
    icon_label_button, ui_unit,
};
use crate::art::{self, Anim, Mood};
use crate::bank::{self, Shelf};
use crate::gfx::{rgb, rgba};
use crate::platform::{self, Ask};
use crate::render::{self, View};
use crate::share;
use crate::sim::{
    Ent, FAIRY_RADIUS, Game, Kind, Level, Material, Piece, PieceKind, PigKind, Shape, Size, V2,
    WolfKind,
};

/// Positions snap to this grid (meters).
const GRID: f32 = 0.1;
/// Rotation step (10 degrees).
const ANGLE_STEP: f32 = TAU / 36.0;
/// Resize step (meters).
const SIZE_STEP: f32 = 0.2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Earth,
    Blocks,
    Wolves,
    Pigs,
    /// Fairies (and, one day, portals and other magic).
    Magic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Piece(usize),
    Launcher,
}

enum Drag {
    /// A new piece being dragged out of the palette.
    New(Piece),
    /// An existing piece; `grab` is the offset from its center to the finger,
    /// `from` where the finger went down (on screen).
    Move {
        idx: usize,
        grab: V2,
        from: Vec2,
        moved: bool,
    },
    Launcher {
        grab: V2,
        from: Vec2,
        moved: bool,
    },
    /// Panning the view by dragging empty space.
    Pan {
        from: Vec2,
        start: V2,
        moved: bool,
    },
}

/// Pointer travel (screen points) before a press counts as a drag.
const DRAG_SLOP: f32 = 6.0;

pub struct Editor {
    pub level: Level,
    /// The "My levels" entry this saves into (by bank id). `None` until a
    /// remix's first change, which creates the entry.
    pub id: Option<String>,
    undo: Vec<String>,
    redo: Vec<String>,
    selection: Option<Selection>,
    drag: Option<Drag>,
    category: Category,
    material: Material,
    snap: bool,
    /// The level as built for display; rebuilt whenever it changes.
    preview: Game,
    /// Pieces that move by themselves within a couple of seconds.
    wobbly: Vec<usize>,
    /// Code at the last save; changes are saved automatically.
    saved: String,
    /// Camera: world point at the center of the scene (None = default) and zoom
    /// (1 = the play area's full width).
    center: Option<V2>,
    zoom: f32,
}

const DEFAULT_PIGS: [PigKind; 3] = [PigKind::Grunt, PigKind::Grunt, PigKind::Grunt];

/// A name for a new level, like "Soggy Summit".
pub fn random_name() -> String {
    const MOOD: [&str; 20] = [
        "Muddy", "Wobbly", "Soggy", "Grumpy", "Rickety", "Windy", "Crumbly", "Sneaky", "Bumpy",
        "Prickly", "Shaky", "Lumpy", "Dizzy", "Squishy", "Frosty", "Rumbly", "Tipsy", "Gusty",
        "Creaky", "Stubborn",
    ];
    const PLACE: [&str; 20] = [
        "Meadow", "Ridge", "Gulch", "Summit", "Hollow", "Fort", "Tower", "Hideout", "Burrow",
        "Bluff", "Canyon", "Pen", "Barn", "Bridge", "Den", "Hill", "Stack", "Pile", "Keep",
        "Lookout",
    ];
    let pick = |words: &[&'static str]| words[macroquad::rand::gen_range(0, words.len())];
    format!("{} {}", pick(&MOOD), pick(&PLACE))
}

impl Editor {
    /// Opens `level` for editing. With an `id` it's one of my saved levels
    /// (its undo history comes back too); otherwise it's a remix, credited
    /// to its old author.
    pub fn open(mut level: Level, id: Option<String>, me: &str) -> Self {
        if id.is_none() {
            if !level.author.is_empty() && level.author != me && level.based_on.is_empty() {
                level.based_on.clone_from(&level.author);
            }
            me.clone_into(&mut level.author);
        }
        let saved = share::encode(&level);
        let undo = id.as_deref().map_or_else(Vec::new, bank::load_history);
        let mut ed = Editor {
            preview: Game::new(&level),
            level,
            id,
            undo,
            redo: Vec::new(),
            selection: None,
            drag: None,
            category: Category::Blocks,
            material: Material::Wood,
            snap: true,
            wobbly: Vec::new(),
            saved,
            center: None,
            zoom: 1.0,
        };
        ed.rebuild(true);
        ed
    }

    /// A fresh level with a random name. It's unsaved (`saved` is empty),
    /// so the first autosave puts it in My levels straight away.
    pub fn new_level(me: &str) -> Self {
        let level = Level {
            name: random_name(),
            author: me.to_owned(),
            pigs: DEFAULT_PIGS.to_vec(),
            ..Level::default()
        };
        let mut ed = Self::open(level, None, me);
        ed.saved.clear();
        ed
    }

    /// Dev aid for screenshots: preselect a piece and a palette category.
    pub fn dev_setup(&mut self, select: Option<usize>, category: Option<&str>) {
        self.selection = select.map(Selection::Piece);
        self.category = match category {
            Some("earth") => Category::Earth,
            Some("wolves") => Category::Wolves,
            Some("pigs") => Category::Pigs,
            Some("magic") => Category::Magic,
            _ => Category::Blocks,
        };
    }

    fn code(&self) -> String {
        share::encode(&self.level)
    }

    /// Call before changing the level, so the change can be undone.
    fn checkpoint(&mut self) {
        let code = self.code();
        if self.undo.last() != Some(&code) {
            self.undo.push(code);
        }
        self.redo.clear();
    }

    fn rebuild(&mut self, check_wobble: bool) {
        self.preview = Game::new(&self.level);
        if check_wobble {
            self.wobbly = wobbly_pieces(&self.level);
        }
    }

    fn step_history(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(code) = from.pop() else { return };
        to.push(share::encode(&self.level));
        if let Ok(level) = share::decode(&code) {
            self.level = level;
        }
        self.selection = None;
        self.rebuild(true);
    }

    fn selected_piece(&self) -> Option<usize> {
        match self.selection {
            Some(Selection::Piece(i)) if i < self.level.pieces.len() => Some(i),
            _ => None,
        }
    }

    /// Applies `f` to the selected piece as one undoable change.
    fn edit_selected(&mut self, f: impl FnOnce(&mut Piece)) {
        let Some(i) = self.selected_piece() else {
            return;
        };
        self.checkpoint();
        f(&mut self.level.pieces[i]);
        if self.snap {
            let p = self.level.pieces[i];
            self.level.pieces[i].y = drop_y(&self.level, &p, Some(i));
        }
        self.rebuild(true);
    }
}

// --- Geometry ---

/// A piece's outline polygon in world space (circles approximated).
fn outline(p: &Piece) -> Vec<V2> {
    let c = V2::new(p.x, p.y);
    match p.size {
        Size::Box { w, h } => {
            let (s, co) = p.angle.sin_cos();
            [(-w, -h), (w, -h), (w, h), (-w, h)]
                .iter()
                .map(|&(x, y)| {
                    let (x, y) = (x / 2.0, y / 2.0);
                    c + V2::new(x * co - y * s, x * s + y * co)
                })
                .collect()
        }
        Size::Ball { r } => (0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * TAU;
                c + V2::new(a.cos(), a.sin()) * r
            })
            .collect(),
    }
}

/// Lowest and highest y of a convex polygon along the vertical line at `x`.
fn vertical_span(poly: &[V2], x: f32) -> Option<(f32, f32)> {
    let mut span: Option<(f32, f32)> = None;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (lo, hi) = if a.x <= b.x { (a, b) } else { (b, a) };
        if x < lo.x || x > hi.x || (hi.x - lo.x).abs() < 1e-6 {
            continue;
        }
        let y = lo.y + (hi.y - lo.y) * (x - lo.x) / (hi.x - lo.x);
        span = Some(span.map_or((y, y), |(mn, mx)| (mn.min(y), mx.max(y))));
    }
    span
}

fn bounds(poly: &[V2]) -> (V2, V2) {
    poly.iter().fold(
        (V2::splat(f32::INFINITY), V2::splat(f32::NEG_INFINITY)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)),
    )
}

/// The height `p` would come to rest at if dropped straight down onto the
/// ground or whatever is beneath it (pieces whose surface is below its center).
fn drop_y(level: &Level, p: &Piece, exclude: Option<usize>) -> f32 {
    if p.kind == PieceKind::Fairy {
        // Fairies hover right where they're put.
        return p.y;
    }
    let at_zero = Piece { y: 0.0, ..*p };
    let shape = outline(&at_zero);
    let (lo, hi) = bounds(&shape);
    let others: Vec<Vec<V2>> = level
        .pieces
        .iter()
        .enumerate()
        .filter(|&(i, _)| Some(i) != exclude)
        .map(|(_, o)| outline(o))
        .collect();
    let mut rest = f32::NEG_INFINITY;
    for k in 0..=16 {
        let x = lo.x + (hi.x - lo.x) * (0.01 + 0.98 * k as f32 / 16.0);
        let Some((bottom, _)) = vertical_span(&shape, x) else {
            continue;
        };
        let support = others
            .iter()
            .filter_map(|o| vertical_span(o, x).map(|(_, top)| top))
            .filter(|&top| top <= p.y + 0.05)
            .fold(0.0_f32, f32::max);
        rest = rest.max(support - bottom);
    }
    if rest.is_finite() { rest } else { p.y }
}

/// Where the launcher's base lands if dropped at `x`, from height `y`.
fn launcher_drop(level: &Level, x: f32, y: f32) -> f32 {
    level
        .pieces
        .iter()
        .filter_map(|o| vertical_span(&outline(o), x).map(|(_, top)| top))
        .filter(|&top| top <= y + 0.6)
        .fold(0.0_f32, f32::max)
}

fn contains(p: &Piece, w: V2, margin: f32) -> bool {
    let d = w - V2::new(p.x, p.y);
    match p.size {
        Size::Box { w, h } => {
            let (s, c) = (-p.angle).sin_cos();
            let local = V2::new(d.x * c - d.y * s, d.x * s + d.y * c);
            local.x.abs() <= w / 2.0 + margin && local.y.abs() <= h / 2.0 + margin
        }
        Size::Ball { r } => d.length() <= r + margin,
    }
}

fn snap(v: f32) -> f32 {
    (v / GRID).round() * GRID
}

/// Runs the level's physics briefly (no pigs) and reports pieces that move.
fn wobbly_pieces(level: &Level) -> Vec<usize> {
    let mut g = Game::new(level);
    let start: Vec<(V2, f32)> = g.ents.iter().map(|e| (e.pos, e.angle)).collect();
    for _ in 0..150 {
        g.update(1.0 / 60.0);
    }
    // Entities are created in the same order as the level's pieces.
    g.ents
        .iter()
        .zip(start)
        .enumerate()
        .filter(|(_, (e, (pos, angle)))| {
            !e.alive || (e.pos - *pos).length() > 0.08 || (e.angle - angle).abs() > 0.05
        })
        .map(|(i, _)| i)
        .collect()
}

// --- Drawing helpers ---

/// Draws a single piece (outside any physics world).
pub fn draw_piece(view: &View, p: &Piece, time: f32, seed: u32) {
    let pos = V2::new(p.x, p.y);
    let shape = match p.size {
        Size::Box { w, h } => Shape::Box {
            hw: w / 2.0,
            hh: h / 2.0,
        },
        Size::Ball { r } => Shape::Ball { r },
    };
    match p.kind {
        PieceKind::Wolf(w) => {
            let anim = Anim::new(time, seed, Mood::Idle, V2::new(-1.0, 0.0));
            art::wolf(view, w, pos, p.angle, p.size.radius(), 1.0, 0.0, &anim);
        }
        PieceKind::Fairy => {
            let anim = Anim::new(time, seed, Mood::Idle, V2::new(-1.0, 0.0));
            art::fairy(view, pos, p.size.radius(), &anim, false);
        }
        kind => {
            let kind = match kind {
                PieceKind::Terrain => Kind::Rock,
                PieceKind::Block(m) => Kind::Block(m),
                _ => Kind::Block(Material::Stone),
            };
            render::block(view, &Ent::preview(kind, shape, pos, p.angle, seed));
        }
    }
}

/// A button with a little picture and a word: side by side on wide buttons,
/// the word under the picture on squarer ones.
fn picture_button(
    rect: Rect,
    label: &str,
    input: &Input,
    accent: Color,
    picture: impl FnOnce(Rect),
) -> bool {
    button_bg(rect, accent);
    let sq = rect.h.min(rect.w);
    let words = rect.w > rect.h * 2.0;
    let w = measure_text(label, None, 100, 1.0).width / 100.0;
    if words {
        let pic = Rect::new(rect.x + sq * 0.08, rect.y, sq, sq);
        picture(pic);
        let x0 = pic.right() + sq * 0.08;
        let room = rect.right() - x0 - sq * 0.12;
        let size = (rect.h * 0.45).min(room / w.max(0.01));
        draw_text(label, x0, rect.center().y + size * 0.35, size, WHITE);
    } else {
        let ph = rect.h * 0.72;
        let pw = ph.min(rect.w);
        picture(Rect::new(rect.center().x - pw / 2.0, rect.y, pw, ph));
        let size = (rect.h * 0.24).min(rect.w * 0.9 / w.max(0.01));
        render::text_centered(
            label,
            rect.center().x,
            rect.bottom() - rect.h * 0.08,
            size,
            WHITE,
        );
    }
    input.pressed.is_some_and(|p| rect.contains(p))
}

/// The picture on a palette category's tab.
fn category_picture(cat: Category, pic: Rect, time: f32) {
    let pic = Rect::new(
        pic.x + pic.w * 0.1,
        pic.y + pic.h * 0.1,
        pic.w * 0.8,
        pic.h * 0.8,
    );
    let ball = |r: f32| Size::Ball { r };
    let p = match cat {
        Category::Earth => piece(PieceKind::Terrain, Size::Box { w: 3.0, h: 2.0 }, 0.0),
        Category::Blocks => piece(
            PieceKind::Block(Material::Wood),
            Size::Box { w: 1.0, h: 1.0 },
            0.0,
        ),
        Category::Wolves => piece(PieceKind::Wolf(WolfKind::Wolf), ball(0.5), 0.0),
        Category::Magic => piece(PieceKind::Fairy, ball(FAIRY_RADIUS), 0.0),
        Category::Pigs => {
            let k = PigKind::Grunt;
            let view = View {
                s: pic.h * 0.42 / k.radius(),
                ox: pic.center().x,
                oy: pic.center().y,
                mirror: None,
            };
            art::pig(
                &view,
                k,
                V2::ZERO,
                0.0,
                &Anim::new(time, 4, Mood::Idle, V2::X),
            );
            return;
        }
    };
    draw_piece(&thumb_view(pic, &p), &p, time, 2);
}

/// A view that fits a piece (centered at the origin) inside `rect`.
fn thumb_view(rect: Rect, p: &Piece) -> View {
    let (lo, hi) = bounds(&outline(p));
    let size = hi - lo;
    let s = (rect.w * 0.78 / size.x.max(0.1)).min(rect.h * 0.7 / size.y.max(0.1));
    View {
        s,
        ox: rect.center().x,
        oy: rect.center().y,
        mirror: None,
    }
}

fn outline_screen(view: &View, p: &Piece, color: Color, width: f32) {
    let pts: Vec<Vec2> = outline(p).iter().map(|&w| view.v(w)).collect();
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        draw_line(a.x, a.y, b.x, b.y, width, color);
    }
}

fn piece(kind: PieceKind, size: Size, angle: f32) -> Piece {
    Piece {
        kind,
        x: 0.0,
        y: 0.0,
        angle,
        size,
    }
}

fn palette(cat: Category, mat: Material) -> Vec<Piece> {
    let b = |w: f32, h: f32| Size::Box { w, h };
    let ball = |r: f32| Size::Ball { r };
    let block = PieceKind::Block(mat);
    match cat {
        Category::Earth => vec![
            piece(PieceKind::Terrain, b(3.0, 2.0), 0.0),
            piece(PieceKind::Terrain, b(6.0, 4.0), 0.0),
            piece(PieceKind::Terrain, b(4.0, 0.6), 0.0),
            piece(PieceKind::Terrain, b(1.0, 4.0), 0.0),
            piece(PieceKind::Terrain, b(5.0, 0.6), ANGLE_STEP * 3.0),
            piece(PieceKind::Terrain, b(5.0, 0.6), -ANGLE_STEP * 3.0),
        ],
        Category::Blocks => vec![
            piece(block, b(0.4, 2.0), 0.0),
            piece(block, b(0.4, 3.0), 0.0),
            piece(block, b(3.0, 0.4), 0.0),
            piece(block, b(5.0, 0.4), 0.0),
            piece(block, b(0.8, 0.8), 0.0),
            piece(block, b(1.6, 1.6), 0.0),
            piece(PieceKind::Block(Material::Tnt), b(0.8, 0.8), 0.0),
            piece(PieceKind::Boulder, ball(0.7), 0.0),
        ],
        Category::Wolves => [
            WolfKind::Pup,
            WolfKind::Wolf,
            WolfKind::Helmet,
            WolfKind::Boss,
        ]
        .into_iter()
        .map(|w| piece(PieceKind::Wolf(w), ball(w.radius()), 0.0))
        .collect(),
        Category::Magic => vec![piece(PieceKind::Fairy, ball(FAIRY_RADIUS), 0.0)],
        Category::Pigs => Vec::new(),
    }
}

const PIG_KINDS: [PigKind; 7] = [
    PigKind::Grunt,
    PigKind::Dasher,
    PigKind::Boomer,
    PigKind::Hog,
    PigKind::Litter,
    PigKind::Thumper,
    PigKind::Puffer,
];

const MATERIALS: [(Material, &str); 4] = [
    (Material::Straw, "Straw"),
    (Material::Wood, "Wood"),
    (Material::Brick, "Brick"),
    (Material::Ice, "Ice"),
];

/// What the editor wants after a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditNav {
    Stay,
    Leave,
    Test,
}

/// Screen layout of the editor's controls.
struct Layout {
    top: Rect,
    palette: Rect,
    tools: Option<Rect>,
}

/// The editor's camera: the whole play area, fitted between its top bar and
/// palette so nothing hides under the controls.
fn scene_area() -> Rect {
    let l = Layout::new(false);
    Rect::new(
        0.0,
        l.top.bottom(),
        screen_width(),
        l.palette.y - l.top.bottom(),
    )
}

impl Editor {
    /// The editor camera. By default the play area's full width shows, with
    /// the ground along the bottom; +/- buttons, the wheel and dragging empty
    /// space zoom and pan.
    pub fn view(&self, mirrored: bool) -> View {
        let area = scene_area();
        let s = area.w / render::VIEW_W * self.zoom;
        let c = self.center.unwrap_or_else(|| Self::default_center(area, s));
        View {
            s,
            ox: area.center().x - c.x * s,
            oy: area.center().y + c.y * s,
            mirror: mirrored.then_some(screen_width()),
        }
    }

    fn default_center(area: Rect, s: f32) -> V2 {
        V2::new(
            render::VIEW_X0 + render::VIEW_W / 2.0,
            render::VIEW_Y0 + area.h / s / 2.0,
        )
    }

    fn zoom_by(&mut self, factor: f32) {
        let area = scene_area();
        let s = area.w / render::VIEW_W * self.zoom;
        let c = self.center.unwrap_or_else(|| Self::default_center(area, s));
        self.center = Some(c);
        self.zoom = (self.zoom * factor).clamp(0.6, 5.0);
    }
}

/// The +/- zoom buttons (bottom-left of the scene).
fn zoom_rects() -> (Rect, Rect) {
    let u = ui_unit();
    let l = Layout::new(false);
    let bs = u * 0.07;
    let y = l.palette.y - bs - u * 0.02;
    (
        Rect::new(u * 0.02, y, bs, bs),
        Rect::new(u * 0.02 + bs * 1.2, y, bs, bs),
    )
}

impl Layout {
    fn new(has_tools: bool) -> Self {
        let (sw, sh) = (screen_width(), screen_height());
        let u = ui_unit();
        let ph = u * 0.24;
        Layout {
            top: Rect::new(0.0, 0.0, sw, u * 0.155),
            palette: Rect::new(0.0, sh - ph, sw, ph),
            tools: has_tools
                .then(|| Rect::new(sw - u * 0.2, u * 0.165, u * 0.2, sh - ph - u * 0.175)),
        }
    }
    fn over_ui(&self, p: Vec2) -> bool {
        let (zin, zout) = zoom_rects();
        self.top.contains(p)
            || self.palette.contains(p)
            || self.tools.is_some_and(|t| t.contains(p))
            || zin.contains(p)
            || zout.contains(p)
    }
}

impl App {
    pub(super) fn open_editor(&mut self, ed: Editor) {
        self.editor = Some(Box::new(ed));
        self.screen = Screen::Edit;
    }

    pub(super) fn edit(&mut self, input: &Input, view: &View) {
        let Some(mut ed) = self.editor.take() else {
            self.screen = Screen::Menu;
            return;
        };
        match self.edit_frame(&mut ed, input, view) {
            EditNav::Stay => self.editor = Some(ed),
            EditNav::Leave => {
                platform::save(super::EDITING_KEY, "");
                self.screen = Screen::Menu;
                self.tab = super::menu::Tab::Mine;
            }
            EditNav::Test => {
                // Play it; the in-game menu button comes back to the editor.
                let level = ed.level.clone();
                self.editor = Some(ed);
                self.begin(Source::Editor, level);
            }
        }
    }

    fn edit_frame(&mut self, ed: &mut Editor, input: &Input, view: &View) -> EditNav {
        // Answers typed into prompts (name, author).
        if let Some(name) = self.answer(Ask::LevelName) {
            ed.checkpoint();
            ed.level.name = name;
            ed.rebuild(false);
        }
        if let Some(me) = self.answer(Ask::AuthorName) {
            self.set_me(&me);
            ed.checkpoint();
            ed.level.author = me;
            ed.rebuild(false);
        }

        let layout = Layout::new(ed.selection.is_some());
        let blocked = Input::default();
        let ui_input = if ed.drag.is_some() { &blocked } else { input };
        Self::world_input(ed, input, view, &layout);

        // Scene
        self.draw_world(&ed.preview, view);
        for &i in &ed.wobbly {
            if let Some(p) = ed.level.pieces.get(i) {
                outline_screen(view, p, rgba(230, 60, 50, 0.9), 3.0);
            }
        }
        match ed.selection {
            Some(Selection::Piece(i)) => {
                if let Some(p) = ed.level.pieces.get(i) {
                    outline_screen(view, p, rgb(255, 220, 60), 4.0);
                }
            }
            Some(Selection::Launcher) => {
                let b = view.v(ed.level.launcher.base());
                let top = view.v(ed.level.launcher.pouch());
                let w = view.s * 0.8;
                draw_rectangle_lines(
                    b.x - w / 2.0,
                    top.y - w * 0.3,
                    w,
                    b.y - top.y + w * 0.3,
                    4.0,
                    rgb(255, 220, 60),
                );
            }
            None => {}
        }
        if let Some(Drag::New(p)) = &ed.drag {
            draw_piece(view, p, self.time, 7);
        }

        // Controls
        let (zin, zout) = zoom_rects();
        if button(zin, "+", ui_input, SLATE) {
            ed.zoom_by(1.25);
        }
        if button(zout, "-", ui_input, SLATE) {
            ed.zoom_by(0.8);
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.0 && !layout.over_ui(input.pos) {
            ed.zoom_by(if wheel > 0.0 { 1.1 } else { 1.0 / 1.1 });
        }
        let nav = self.top_bar(ed, ui_input, &layout);
        self.palette_bar(ed, ui_input, &layout);
        if let Some(rect) = layout.tools {
            Self::tool_bar(ed, ui_input, rect);
        }
        if !ed.wobbly.is_empty() {
            let u = ui_unit();
            let y = layout.top.bottom() + u * 0.015;
            let (w, h) = (u * 0.75, u * 0.05);
            let x = (screen_width() - w) / 2.0;
            draw_rectangle(x, y, w, h, rgba(120, 30, 25, 0.75));
            render::text_centered(
                "Red-outlined pieces fall over by themselves",
                screen_width() / 2.0,
                y + h * 0.7,
                u * 0.03,
                WHITE,
            );
        }
        self.autosave(ed);
        nav
    }

    /// Saves the level (and its undo history) whenever it has changed, so
    /// nothing is lost if the app is closed. Mid-drag changes wait for the drop.
    pub(super) fn autosave(&mut self, ed: &mut Editor) {
        if ed.drag.is_some() {
            return;
        }
        let code = ed.code();
        if code == ed.saved {
            return;
        }
        let id = match &ed.id {
            Some(id) => self.bank.update(id, &ed.level),
            None => self
                .bank
                .add(&ed.level, Shelf::Mine)
                .or_else(|| self.bank.mine_with_code(&code))
                .unwrap_or_default(),
        };
        bank::save_history(&id, &ed.undo);
        platform::save(super::EDITING_KEY, &id);
        ed.id = Some(id);
        ed.saved = code;
    }

    /// Dragging pieces and the launcher around the scene, and panning.
    fn world_input(ed: &mut Editor, input: &Input, view: &View, layout: &Layout) {
        Self::start_drag(ed, input, view, layout);
        Self::follow_drag(ed, input, view);
        Self::end_drag(ed, input, layout);
    }

    fn start_drag(ed: &mut Editor, input: &Input, view: &View, layout: &Layout) {
        let world = |p: Vec2| view.unproject(p);
        // Start a drag on press (outside the controls).
        if let Some(p) = input.pressed
            && !layout.over_ui(p)
        {
            let w = world(p);
            // Generous, screen-sized touch targets however far zoomed out.
            let margin = 14.0 / view.s;
            let launcher_hit = {
                let base = ed.level.launcher.base();
                let half_w = 0.6_f32.max(margin * 1.5);
                (w.x - base.x).abs() < half_w
                    && w.y > base.y - margin
                    && w.y < base.y + crate::sim::SLING_HEIGHT + 0.5 + margin
            };
            let hit = (0..ed.level.pieces.len())
                .rev()
                .find(|&i| contains(&ed.level.pieces[i], w, margin));
            if let Some(i) = hit {
                let c = V2::new(ed.level.pieces[i].x, ed.level.pieces[i].y);
                ed.selection = Some(Selection::Piece(i));
                ed.drag = Some(Drag::Move {
                    idx: i,
                    grab: w - c,
                    from: p,
                    moved: false,
                });
            } else if launcher_hit {
                ed.selection = Some(Selection::Launcher);
                ed.drag = Some(Drag::Launcher {
                    grab: w - ed.level.launcher.base(),
                    from: p,
                    moved: false,
                });
            } else {
                let area = scene_area();
                let start = ed
                    .center
                    .unwrap_or_else(|| Editor::default_center(area, view.s));
                ed.drag = Some(Drag::Pan {
                    from: p,
                    start,
                    moved: false,
                });
            }
        }
    }

    fn follow_drag(ed: &mut Editor, input: &Input, view: &View) {
        let world = |p: Vec2| view.unproject(p);
        // Follow the finger.
        if let Some(cur) = input.down {
            let w = world(cur);
            match &mut ed.drag {
                Some(Drag::New(p)) => {
                    p.x = snap(w.x);
                    p.y = snap(w.y);
                }
                Some(Drag::Move {
                    idx,
                    grab,
                    from,
                    moved,
                }) => {
                    let (idx, grab) = (*idx, *grab);
                    if !*moved {
                        if cur.distance(*from) < DRAG_SLOP {
                            return;
                        }
                        *moved = true;
                        ed.checkpoint();
                    }
                    if let Some(p) = ed.level.pieces.get_mut(idx) {
                        p.x = snap(w.x - grab.x);
                        p.y = snap(w.y - grab.y);
                    }
                    ed.rebuild(false);
                }
                Some(Drag::Launcher { grab, from, moved }) => {
                    let grab = *grab;
                    if !*moved {
                        if cur.distance(*from) < DRAG_SLOP {
                            return;
                        }
                        *moved = true;
                        ed.checkpoint();
                    }
                    ed.level.launcher.x = snap(w.x - grab.x);
                    ed.level.launcher.y = snap(w.y - grab.y);
                    ed.rebuild(false);
                }
                Some(Drag::Pan { from, start, moved }) => {
                    let d = cur - *from;
                    if *moved || d.length() >= DRAG_SLOP {
                        *moved = true;
                        let dx = if view.mirror.is_some() { d.x } else { -d.x };
                        ed.center = Some(*start + V2::new(dx / view.s, d.y / view.s));
                    }
                }
                None => {}
            }
        }
    }

    fn end_drag(ed: &mut Editor, input: &Input, layout: &Layout) {
        // Drop.
        if input.released {
            match ed.drag.take() {
                Some(Drag::New(mut p)) => {
                    let over_palette = layout.palette.contains(input.pos);
                    if !over_palette {
                        ed.checkpoint();
                        if ed.snap {
                            p.y = drop_y(&ed.level, &p, None);
                        }
                        ed.level.pieces.push(p);
                        ed.selection = Some(Selection::Piece(ed.level.pieces.len() - 1));
                        ed.rebuild(true);
                    }
                }
                Some(Drag::Move {
                    idx, moved: true, ..
                }) => {
                    if ed.snap
                        && let Some(&p) = ed.level.pieces.get(idx)
                    {
                        ed.level.pieces[idx].y = drop_y(&ed.level, &p, Some(idx));
                    }
                    ed.rebuild(true);
                }
                Some(Drag::Pan { moved: false, .. }) => {
                    // A tap on empty space deselects.
                    ed.selection = None;
                }
                Some(Drag::Launcher { moved: true, .. }) => {
                    if ed.snap {
                        let l = ed.level.launcher;
                        ed.level.launcher.y = launcher_drop(&ed.level, l.x, l.y);
                    }
                    ed.rebuild(true);
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn top_bar(&mut self, ed: &mut Editor, input: &Input, layout: &Layout) -> EditNav {
        let u = ui_unit();
        let sw = screen_width();
        let bs = u * 0.07;
        let y = u * 0.02;
        let mut x = u * 0.02;
        let mut next = |w: f32| {
            let r = Rect::new(x, y, w, bs);
            x += w + u * 0.015;
            r
        };
        draw_rectangle(
            layout.top.x,
            layout.top.y,
            layout.top.w,
            layout.top.h,
            Color::new(0.0, 0.0, 0.0, 0.18),
        );

        let mut nav = EditNav::Stay;
        if icon_label_button(next(bs * 2.2), Icon::Back, "Back", input, SLATE) {
            nav = EditNav::Leave;
        }
        let undo_color = if ed.undo.is_empty() {
            Color::new(0.3, 0.3, 0.35, 0.6)
        } else {
            SLATE
        };
        if icon_label_button(next(bs * 2.2), Icon::Undo, "Undo", input, undo_color) {
            ed.step_history(true);
        }
        let redo_color = if ed.redo.is_empty() {
            Color::new(0.3, 0.3, 0.35, 0.6)
        } else {
            SLATE
        };
        if icon_label_button(next(bs * 2.2), Icon::Redo, "Redo", input, redo_color) {
            ed.step_history(false);
        }
        if icon_label_button(
            next(bs * 2.2),
            Icon::Magnet,
            "Snap",
            input,
            if ed.snap { ORANGE } else { SLATE },
        ) {
            ed.snap = !ed.snap;
        }

        let left_end = x;

        // Right side: Test, Save copy, Share (laid out right to left). There's
        // no Save: every change is saved as it's made.
        let mut rx = sw - u * 0.02;
        let mut prev = |w: f32| {
            rx -= w;
            let r = Rect::new(rx, y, w, bs);
            rx -= u * 0.015;
            r
        };
        let share_r = prev(bs);
        let copy_r = ed.id.is_some().then(|| prev(bs * 3.6));
        let test_r = prev(bs * 2.2);
        let right_start = rx;

        // Name and author fill the space between, tap to change.
        let name_r = Rect::new(
            left_end,
            y,
            (right_start - left_end).max(u * 0.2),
            bs * 0.62,
        );
        let name = if ed.level.name.is_empty() {
            "Untitled"
        } else {
            &ed.level.name
        };
        draw_rectangle(
            name_r.x,
            name_r.y,
            name_r.w,
            name_r.h,
            Color::new(1.0, 1.0, 1.0, 0.15),
        );
        render::text_centered(
            name,
            name_r.center().x,
            name_r.y + name_r.h * 0.78,
            u * 0.04,
            WHITE,
        );
        platform::prompt_hotspot(name_r, Ask::LevelName, "Level name", &ed.level.name);
        if input.pressed.is_some_and(|p| name_r.contains(p)) {
            self.ask_natively(Ask::LevelName, "Level name", &ed.level.name);
        }
        let author_r = Rect::new(name_r.x, name_r.bottom() + u * 0.004, name_r.w, bs * 0.5);
        let by = if ed.level.author.is_empty() {
            "by ... (tap to add your name)".to_owned()
        } else if ed.level.based_on.is_empty() {
            format!("by {}", ed.level.author)
        } else {
            format!("by {} (remix of {})", ed.level.author, ed.level.based_on)
        };
        render::text_centered(
            &by,
            author_r.center().x,
            author_r.y + author_r.h * 0.75,
            u * 0.026,
            rgb(255, 236, 200),
        );
        platform::prompt_hotspot(
            author_r,
            Ask::AuthorName,
            "Your name (shown on levels you share)",
            &self.me,
        );
        if input.pressed.is_some_and(|p| author_r.contains(p)) {
            let me = self.me.clone();
            self.ask_natively(Ask::AuthorName, "Your name", &me);
        }

        platform::share_hotspot(share_r, &super::menu::share_link(&ed.level));
        if icon_button(share_r, Icon::Share, input, SLATE) {
            platform::activate(platform::Hotspot::Share);
        }
        if let Some(copy_r) = copy_r
            && icon_label_button(copy_r, Icon::Copy, "Save copy", input, SLATE)
        {
            // Branch off: the current version stays as it is, and editing
            // carries on in a copy (with the same undo history).
            ed.level.name = format!("{} (copy)", ed.level.name);
            ed.id = None;
            ed.saved.clear();
            self.autosave(ed);
            self.toast("Copied - you're now editing the copy");
        }
        if icon_label_button(test_r, Icon::Play, "Test", input, ORANGE) {
            ed.selection = None;
            nav = EditNav::Test;
        }
        nav
    }

    fn palette_bar(&mut self, ed: &mut Editor, input: &Input, layout: &Layout) {
        let u = ui_unit();
        let r = layout.palette;
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.08, 0.06, 0.55));
        let pad = u * 0.02;

        // Category tabs down the left edge.
        let cats = [
            (Category::Earth, "Earth"),
            (Category::Blocks, "Blocks"),
            (Category::Wolves, "Wolves"),
            (Category::Pigs, "Pigs"),
            (Category::Magic, "Magic"),
        ];
        // Two columns of picture tabs.
        let tw = u * 0.24;
        let rows = cats.len().div_ceil(2) as f32;
        let cw = (tw - pad) / 2.0;
        let th = (r.h - pad * (rows + 1.0)) / rows;
        for (i, (cat, label)) in cats.iter().enumerate() {
            let (row, col) = ((i / 2) as f32, (i % 2) as f32);
            let tr = Rect::new(
                r.x + pad + col * (cw + pad),
                r.y + pad + row * (th + pad),
                cw,
                th,
            );
            let color = if ed.category == *cat { ORANGE } else { SLATE };
            if picture_button(tr, label, input, color, |pic| {
                category_picture(*cat, pic, self.time);
            }) {
                ed.category = *cat;
            }
        }
        let x0 = r.x + tw + pad * 2.0;

        if ed.category == Category::Pigs {
            self.pig_palette(ed, input, Rect::new(x0, r.y, r.w - x0, r.h));
            return;
        }

        // Material chips (blocks only).
        let mut items_y = r.y + pad;
        if ed.category == Category::Blocks {
            let ch = u * 0.05;
            for (i, (m, label)) in MATERIALS.iter().enumerate() {
                let cr = Rect::new(x0 + i as f32 * (u * 0.15 + pad), r.y + pad, u * 0.15, ch);
                let color = if ed.material == *m { ORANGE } else { SLATE };
                let m_ = *m;
                if picture_button(cr, label, input, color, |pic| {
                    let p = piece(PieceKind::Block(m_), Size::Box { w: 1.0, h: 1.0 }, 0.0);
                    draw_piece(&thumb_view(pic, &p), &p, 0.0, 5);
                }) {
                    ed.material = *m;
                    // Also restyle the selected block.
                    if let Some(i) = ed.selected_piece()
                        && matches!(ed.level.pieces[i].kind, PieceKind::Block(old) if !matches!(old, Material::Tnt | Material::Stone))
                    {
                        let m = *m;
                        ed.edit_selected(|p| p.kind = PieceKind::Block(m));
                    }
                }
            }
            items_y += ch + pad;
        }

        // Pieces: press to drag one out.
        let items = palette(ed.category, ed.material);
        let ih = r.bottom() - pad - items_y;
        let iw = ih.min((r.w - x0 - pad) / items.len().max(1) as f32 - pad);
        for (i, p) in items.iter().enumerate() {
            let ir = Rect::new(x0 + i as f32 * (iw + pad), items_y, iw, ih);
            draw_rectangle(ir.x, ir.y, ir.w, ir.h, Color::new(1.0, 1.0, 1.0, 0.12));
            draw_piece(&thumb_view(ir, p), p, self.time, 3);
            if input.pressed.is_some_and(|q| ir.contains(q)) {
                ed.selection = None;
                ed.drag = Some(Drag::New(*p));
            }
        }
    }

    fn pig_palette(&mut self, ed: &mut Editor, input: &Input, area: Rect) {
        let u = ui_unit();
        let pad = u * 0.02;
        let label_size = u * 0.026;
        draw_text(
            "Tap to add:",
            area.x,
            area.y + pad + label_size,
            label_size,
            WHITE,
        );
        let size = u * 0.062;
        for (i, &k) in PIG_KINDS.iter().enumerate() {
            let r = Rect::new(
                area.x + i as f32 * (size + pad),
                area.y + pad * 1.5 + label_size,
                size,
                size,
            );
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.12));
            let view = View {
                s: size * 0.4 / k.radius(),
                ox: r.center().x,
                oy: r.center().y + size * 0.05,
                mirror: None,
            };
            art::pig(
                &view,
                k,
                V2::ZERO,
                0.0,
                &Anim::new(self.time, 11, Mood::Idle, V2::X),
            );
            if input.pressed.is_some_and(|p| r.contains(p)) {
                ed.checkpoint();
                ed.level.pigs.push(k);
                ed.rebuild(false);
            }
        }
        let y = area.y + pad * 2.0 + label_size + size;
        draw_text(
            "Lineup (tap to remove):",
            area.x,
            y + label_size,
            label_size,
            WHITE,
        );
        let small = u * 0.05;
        let mut remove = None;
        for (i, &k) in ed.level.pigs.iter().enumerate() {
            let r = Rect::new(
                area.x + i as f32 * (small + pad * 0.5),
                y + label_size * 1.3,
                small,
                small,
            );
            if r.right() > area.right() {
                break;
            }
            let view = View {
                s: small * 0.4 / k.radius(),
                ox: r.center().x,
                oy: r.center().y,
                mirror: None,
            };
            art::pig(
                &view,
                k,
                V2::ZERO,
                0.0,
                &Anim::new(
                    self.time,
                    12 + u32::try_from(i).unwrap_or(0),
                    Mood::Idle,
                    V2::X,
                ),
            );
            if input.pressed.is_some_and(|p| r.contains(p)) {
                remove = Some(i);
            }
        }
        if ed.level.pigs.is_empty() {
            draw_text(
                "(no pigs yet)",
                area.x,
                y + label_size * 2.6,
                label_size,
                rgb(255, 200, 190),
            );
        }
        if let Some(i) = remove {
            ed.checkpoint();
            ed.level.pigs.remove(i);
            ed.rebuild(false);
        }
    }

    fn tool_bar(ed: &mut Editor, input: &Input, area: Rect) {
        let u = ui_unit();
        draw_rectangle(
            area.x,
            area.y,
            area.w,
            area.h,
            Color::new(0.0, 0.0, 0.0, 0.25),
        );
        let pad = u * 0.012;
        let bh = ((area.h - pad) / 9.0 - pad).min(u * 0.075);
        let mut y = area.y + pad;
        let mut next = || {
            let r = Rect::new(area.x + pad, y, area.w - pad * 2.0, bh);
            y += bh + pad;
            r
        };
        if ed.selection == Some(Selection::Launcher) {
            let r = next();
            render::text_centered("Launcher:", r.center().x, r.y + bh * 0.65, u * 0.028, WHITE);
            let r = next();
            render::text_centered("drag it", r.center().x, r.y + bh * 0.65, u * 0.028, WHITE);
            return;
        }
        let Some(i) = ed.selected_piece() else { return };
        let round = matches!(ed.level.pieces[i].size, Size::Ball { .. });
        if icon_label_button(next(), Icon::RotateRight, "Turn", input, SLATE) {
            ed.edit_selected(|p| p.angle -= ANGLE_STEP);
        }
        if icon_label_button(next(), Icon::RotateLeft, "Turn", input, SLATE) {
            ed.edit_selected(|p| p.angle += ANGLE_STEP);
        }
        let grow = |dw: f32, dh: f32| {
            move |p: &mut Piece| {
                p.size = match p.size {
                    Size::Box { w, h } => Size::Box {
                        w: (w + dw).max(SIZE_STEP),
                        h: (h + dh).max(SIZE_STEP),
                    },
                    Size::Ball { r } => Size::Ball {
                        r: (r + dw / 2.0).max(SIZE_STEP / 2.0),
                    },
                };
            }
        };
        if round {
            if icon_label_button(next(), Icon::Bigger, "Bigger", input, SLATE) {
                ed.edit_selected(grow(SIZE_STEP, 0.0));
            }
            if icon_label_button(next(), Icon::Smaller, "Smaller", input, SLATE) {
                ed.edit_selected(grow(-SIZE_STEP, 0.0));
            }
        } else {
            if icon_label_button(next(), Icon::Wider, "Wider", input, SLATE) {
                ed.edit_selected(grow(SIZE_STEP, 0.0));
            }
            if icon_label_button(next(), Icon::Narrower, "Narrower", input, SLATE) {
                ed.edit_selected(grow(-SIZE_STEP, 0.0));
            }
            if icon_label_button(next(), Icon::Taller, "Taller", input, SLATE) {
                ed.edit_selected(grow(0.0, SIZE_STEP));
            }
            if icon_label_button(next(), Icon::Shorter, "Shorter", input, SLATE) {
                ed.edit_selected(grow(0.0, -SIZE_STEP));
            }
        }
        if icon_label_button(next(), Icon::Copy, "Copy", input, SLATE) {
            ed.checkpoint();
            let mut p = ed.level.pieces[i];
            p.x += 1.0;
            if ed.snap {
                p.y = drop_y(&ed.level, &p, None);
            }
            ed.level.pieces.push(p);
            ed.selection = Some(Selection::Piece(ed.level.pieces.len() - 1));
            ed.rebuild(true);
        }
        if icon_label_button(next(), Icon::Trash, "Delete", input, rgb(200, 77, 69)) {
            ed.checkpoint();
            ed.level.pieces.remove(i);
            ed.selection = None;
            ed.rebuild(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels;

    fn block(x: f32, y: f32, w: f32, h: f32) -> Piece {
        Piece {
            kind: PieceKind::Block(Material::Wood),
            x,
            y,
            angle: 0.0,
            size: Size::Box { w, h },
        }
    }

    #[test]
    fn drops_onto_the_ground() {
        let level = Level::default();
        let p = block(10.0, 7.0, 1.0, 0.4);
        assert!((drop_y(&level, &p, None) - 0.2).abs() < 1e-4);
    }

    #[test]
    fn drops_onto_whatever_is_below() {
        let level = Level {
            pieces: vec![block(10.0, 1.0, 2.0, 2.0)],
            ..Level::default()
        };
        // Released above the block: lands on its top (y = 2) + half height.
        let p = block(10.5, 6.0, 1.0, 0.4);
        assert!((drop_y(&level, &p, None) - 2.2).abs() < 1e-4);
        // Released beside it: lands on the ground.
        let p = block(14.0, 6.0, 1.0, 0.4);
        assert!((drop_y(&level, &p, None) - 0.2).abs() < 1e-4);
    }

    #[test]
    fn rests_on_the_slope_of_a_ramp() {
        // A 30-degree ramp rising to the right; its surface passes (10, 2).
        let a = ANGLE_STEP * 3.0;
        let ramp = Piece {
            kind: PieceKind::Terrain,
            x: 10.0 + 0.3 * a.sin(),
            y: 2.0 - 0.3 * a.cos(),
            angle: a,
            size: Size::Box { w: 8.0, h: 0.6 },
        };
        let level = Level {
            pieces: vec![ramp],
            ..Level::default()
        };
        let ball = Piece {
            kind: PieceKind::Boulder,
            x: 10.0,
            y: 8.0,
            angle: 0.0,
            size: Size::Ball { r: 0.5 },
        };
        let y = drop_y(&level, &ball, None);
        // Surface height at x = 10 is 2; the ball's center sits ~r/cos(a) above.
        let expect = 2.0 + 0.5 / a.cos();
        assert!((y - expect).abs() < 0.08, "{y} vs {expect}");
    }

    #[test]
    fn floating_pieces_are_reported_as_wobbly() {
        let level = Level {
            pieces: vec![block(10.0, 0.2, 2.0, 0.4), block(14.0, 5.0, 1.0, 1.0)],
            ..Level::default()
        };
        assert_eq!(wobbly_pieces(&level), vec![1]);
    }

    #[test]
    fn built_in_levels_have_no_wobbly_pieces() {
        for lvl in levels::all() {
            assert!(wobbly_pieces(&lvl).is_empty(), "{}", lvl.name);
        }
    }

    #[test]
    fn hit_testing_respects_rotation() {
        let mut p = block(0.0, 0.0, 4.0, 0.4);
        assert!(contains(&p, V2::new(1.8, 0.0), 0.0));
        p.angle = std::f32::consts::FRAC_PI_2;
        assert!(!contains(&p, V2::new(1.8, 0.0), 0.0));
        assert!(contains(&p, V2::new(0.0, 1.8), 0.0));
    }
}
