//! The player's level bank: levels they made and levels friends sent them,
//! saved on the device. Each entry has a permanent id (a level's code changes
//! with every edit), which also keys its undo history.
//!
//! Storage format, one entry per line: `m|f <id> <level code>`. Older lines
//! without an id (`m|f <level code>`) are read fine and get one.

use macroquad::rand;

use crate::platform;
use crate::share::{self, DecodeError};
use crate::sim::Level;

const KEY: &str = "bank";
/// How many earlier versions of a level are kept for undo.
const HISTORY_LEN: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf {
    /// Made (or remixed) by the player.
    Mine,
    /// Received from someone else.
    Friends,
}

pub struct Entry {
    pub id: String,
    pub place: Shelf,
    /// The level code (`L1.…`), kept verbatim.
    pub code: String,
    /// The decoded level, or why it couldn't be read. Entries this version
    /// can't read (e.g. made by a newer game) are kept, not dropped, so they
    /// work again after an update.
    pub level: Result<Level, DecodeError>,
}

#[derive(Default)]
pub struct Bank {
    pub entries: Vec<Entry>,
}

fn new_id() -> String {
    format!("{:08x}", rand::gen_range(0u32, u32::MAX))
}

fn history_key(id: &str) -> String {
    format!("undo.{id}")
}

/// A level's saved undo history (oldest first).
pub fn load_history(id: &str) -> Vec<String> {
    platform::load(&history_key(id))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Saves the most recent part of a level's undo history.
pub fn save_history(id: &str, undo: &[String]) {
    let start = undo.len().saturating_sub(HISTORY_LEN);
    platform::save(&history_key(id), &undo[start..].join("\n"));
}

impl Bank {
    pub fn load() -> Self {
        let text = platform::load(KEY).unwrap_or_default();
        let mut needs_ids = false;
        let entries = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.split(' ');
                let place = match parts.next()? {
                    "m" => Shelf::Mine,
                    "f" => Shelf::Friends,
                    _ => return None,
                };
                let first = parts.next()?;
                let (id, code) = if let Some(code) = parts.next() {
                    (first.to_owned(), code)
                } else {
                    // A line from before entries had ids.
                    needs_ids = true;
                    (new_id(), first)
                };
                Some(Entry {
                    id,
                    place,
                    code: code.to_owned(),
                    level: share::decode(code),
                })
            })
            .collect();
        let bank = Bank { entries };
        if needs_ids {
            bank.save();
        }
        bank
    }

    fn save(&self) {
        let mut text = String::new();
        for e in &self.entries {
            text.push(match e.place {
                Shelf::Mine => 'm',
                Shelf::Friends => 'f',
            });
            text.push(' ');
            text.push_str(&e.id);
            text.push(' ');
            text.push_str(&e.code);
            text.push('\n');
        }
        platform::save(KEY, &text);
    }

    pub fn contains(&self, code: &str) -> bool {
        self.entries.iter().any(|e| e.code == code)
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The id of my saved level with exactly this content, if any.
    pub fn mine_with_code(&self, code: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.place == Shelf::Mine && e.code == code)
            .map(|e| e.id.clone())
    }

    /// Adds a level to a shelf (newest first) and returns its id. Returns
    /// `None` if that shelf already has this exact level.
    pub fn add(&mut self, level: &Level, place: Shelf) -> Option<String> {
        let code = share::encode(level);
        if self
            .entries
            .iter()
            .any(|e| e.code == code && e.place == place)
        {
            return None;
        }
        let id = new_id();
        self.entries.insert(
            0,
            Entry {
                id: id.clone(),
                place,
                level: Ok(level.clone()),
                code,
            },
        );
        self.save();
        Some(id)
    }

    /// Saves `level` over entry `id`, keeping its place in the list (or adds
    /// it to my levels if that entry is gone). Returns the id it's saved under.
    pub fn update(&mut self, id: &str, level: &Level) -> String {
        let code = share::encode(level);
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            if e.code != code {
                e.code = code;
                e.level = Ok(level.clone());
                self.save();
            }
            return id.to_owned();
        }
        self.add(level, Shelf::Mine)
            .or_else(|| self.mine_with_code(&code))
            .unwrap_or_default()
    }

    /// Deletes an entry and its undo history.
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        self.save();
        platform::save(&history_key(id), "");
    }

    pub fn on_shelf(&self, place: Shelf) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |e| e.place == place)
    }
}
