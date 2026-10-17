//! The data the whole program works with: cards, the series and sets they belong to, and decks.
//!
//! A simulator install holds many [`Serie`]s (one per franchise, e.g. "KonoSuba!"). Each serie
//! has one or more [`Set`]s (booster boxes, trial decks…), and each set has its [`Card`]s,
//! keyed by card code (`KS/W49-E001`).

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Height / width of a card picture (the simulator's artwork is 224 × 313 pixels).
pub const CARD_IMAGE_RATIO: f32 = 313.0 / 224.0;

/// The order of the variants matters: it is the order of the card list's colour sort.
/// (`derive(PartialOrd, Ord)` compares enum variants in declaration order.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
    Yellow,
    Blue,
    Red,
    Green,
    Purple,
    All,
    None,
}

impl Color {
    pub fn label(self) -> &'static str {
        match self {
            Color::Yellow => "Yellow",
            Color::Blue => "Blue",
            Color::Red => "Red",
            Color::Green => "Green",
            Color::Purple => "Purple",
            Color::All => "All",
            Color::None => "None",
        }
    }
}

/// The order of the variants matters: characters sort before events, events before climaxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Character,
    Event,
    Climax,
}

impl CardType {
    pub fn label(self) -> &'static str {
        match self {
            CardType::Character => "Character",
            CardType::Event => "Event",
            CardType::Climax => "Climax",
        }
    }

    /// Section title for a group of cards of this type.
    pub fn plural_label(self) -> &'static str {
        match self {
            CardType::Character => "Characters",
            CardType::Event => "Events",
            CardType::Climax => "Climaxes",
        }
    }
}

/// The icon in a card's top-right corner, which takes effect when the card is revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trigger {
    Soul,
    Choice,
    Burn,
    Wind,
    Standby,
    Salvage,
    Book,
    Pant,
    Bar,
    Bag,
}

impl Trigger {
    pub fn label(self) -> &'static str {
        match self {
            Trigger::Bag => "Goldbag",
            Trigger::Bar => "Goldbar",
            Trigger::Book => "Book",
            Trigger::Burn => "Burn",
            Trigger::Choice => "Choice",
            Trigger::Pant => "Pant",
            Trigger::Salvage => "Salvage",
            Trigger::Soul => "Soul",
            Trigger::Standby => "Standby",
            Trigger::Wind => "Wind",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// The card code, e.g. `KS/W49-E001`.
    pub key: String,
    pub card_type: CardType,
    pub name: String,
    /// `None` when no artwork was found; the UI draws a placeholder instead.
    pub image: Option<PathBuf>,
    pub color: Color,
    pub level: u32,
    pub cost: u32,
    pub power: i32,
    pub triggers: Vec<Trigger>,
    pub soul_count: u32,
    pub traits: Vec<String>,
    /// The simulator's script for the card (`Auto:` / `Cont:` / `Act:` blocks). Not shown.
    pub code: String,
    /// The effect text shown to players, one effect per line.
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Set {
    /// Code prefix shared by the set's cards, e.g. `KS/W49`.
    pub key: String,
    pub name: String,
    pub path: PathBuf,
    /// The set's cards by card code. A `BTreeMap` is a map that keeps its keys sorted.
    pub cards: BTreeMap<String, Card>,
}

#[derive(Debug, Clone, Default)]
pub struct Serie {
    pub name: String,
    pub path: PathBuf,
    pub sets: Vec<Set>,
}

/// A deck whose card codes have been matched to loaded cards (see
/// [`crate::card_index::CardIndex::resolve_deck`]). For the deck *file* as saved in the
/// simulator, see [`crate::simulator::deck_files::DeckFile`].
#[derive(Debug, Clone, Default)]
pub struct Deck {
    pub name: String,
    pub date: String,
    /// One entry per copy, in the deck file's order.
    pub cards: Vec<Card>,
    /// Codes of the deck that match no loaded card (another simulator version, a typo…).
    pub unmatched: Vec<String>,
}
