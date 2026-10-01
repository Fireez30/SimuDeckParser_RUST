use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Declaration order matters: it is the sort order used by `SortOrder::Color`.
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

/// Declaration order matters: characters sort before events, events before climaxes.
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
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Color,
    TypeAscending,
    LevelAscending,
    LevelDescending,
    CostAscending,
    CostDescending,
    PowerAscending,
    PowerDescending,
    KeycodeAscending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub key: String,
    pub card_type: CardType,
    pub name: String,
    /// `None` when no artwork was found; the UI draws the placeholder instead.
    pub image: Option<PathBuf>,
    pub color: Color,
    pub level: u32,
    pub cost: u32,
    pub power: i32,
    pub triggers: Vec<Trigger>,
    pub soul_count: u32,
    pub traits: Vec<String>,
    /// Simulator script lines (`Auto:` / `Cont:` / `Act:` blocks).
    pub code: String,
    pub text: String,
}

impl Card {
    /// Plain-text dump of the card, used for debugging and parse comparisons.
    pub fn whole_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, " Card : {} ({}) ", self.key, self.card_type.label());
        let _ = writeln!(s, "    name : {}", self.name);
        let _ = writeln!(s, "    level {} / cost {}", self.level, self.cost);
        let _ = writeln!(s, "    power : {}", self.power);
        let _ = writeln!(s, "    color : {}", self.color.label());
        if !self.triggers.is_empty() {
            s.push_str("    triggers : ");
            for t in &self.triggers {
                s.push_str(t.label());
                s.push(' ');
            }
            s.push('\n');
        }
        let _ = writeln!(s, "    souls : {}", self.soul_count);
        for (i, trait_name) in self.traits.iter().enumerate() {
            let _ = writeln!(s, "trait {i} : {trait_name}");
        }
        let _ = writeln!(s, "    text : {}", self.text);
        let _ = writeln!(s, "    code : {}", self.code);
        let image = self
            .image
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let _ = writeln!(s, "    image : {image}");
        s
    }
}

#[derive(Debug, Clone, Default)]
pub struct Set {
    pub key: String,
    pub name: String,
    pub path: PathBuf,
    pub cards: BTreeMap<String, Card>,
}

impl Set {
    /// Exact lookup, then retry without trailing letters (foil/rarity suffix such as `...-001S`).
    pub fn get_card(&self, code: &str) -> Option<&Card> {
        self.cards.get(code).or_else(|| {
            let stripped = crate::text::remove_trailing_alphas(code);
            if stripped != code {
                self.cards.get(stripped)
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Serie {
    pub name: String,
    pub path: PathBuf,
    pub sets: Vec<Set>,
}

#[derive(Debug, Clone, Default)]
pub struct Deck {
    pub name: String,
    pub date: String,
    pub cards: Vec<Card>,
}
