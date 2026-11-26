//! The player's level bank: levels they made and levels friends sent them,
//! saved on the device as level codes (one per line).

use crate::platform;
use crate::share::{self, DecodeError};
use crate::sim::Level;

const KEY: &str = "bank";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf {
    /// Made (or remixed) by the player.
    Mine,
    /// Received from someone else.
    Friends,
}

pub struct Entry {
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

impl Bank {
    pub fn load() -> Self {
        let text = platform::load(KEY).unwrap_or_default();
        let entries = text
            .lines()
            .filter_map(|line| {
                let (place, code) = line.split_once(' ')?;
                let place = match place {
                    "m" => Shelf::Mine,
                    "f" => Shelf::Friends,
                    _ => return None,
                };
                Some(Entry {
                    place,
                    code: code.to_owned(),
                    level: share::decode(code),
                })
            })
            .collect();
        Bank { entries }
    }

    fn save(&self) {
        let mut text = String::new();
        for e in &self.entries {
            text.push(match e.place {
                Shelf::Mine => 'm',
                Shelf::Friends => 'f',
            });
            text.push(' ');
            text.push_str(&e.code);
            text.push('\n');
        }
        platform::save(KEY, &text);
    }

    pub fn contains(&self, code: &str) -> bool {
        self.entries.iter().any(|e| e.code == code)
    }

    /// Adds a level to a place (newest first). Returns false if it's already there.
    pub fn add(&mut self, level: &Level, place: Shelf) -> bool {
        let code = share::encode(level);
        if self
            .entries
            .iter()
            .any(|e| e.code == code && e.place == place)
        {
            return false;
        }
        self.entries.insert(
            0,
            Entry {
                place,
                level: Ok(level.clone()),
                code,
            },
        );
        self.save();
        true
    }

    pub fn remove(&mut self, code: &str, place: Shelf) {
        self.entries
            .retain(|e| !(e.code == code && e.place == place));
        self.save();
    }

    pub fn on_shelf(&self, place: Shelf) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |e| e.place == place)
    }
}
