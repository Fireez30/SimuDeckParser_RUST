//! `<set folder>/CardData.txt` parser.
//!
//! The file is a sequence of blocks such as:
//! ```text
//! Character: KS/W49-E073
//! Name Megumin
//! Color R
//! Level 3
//! Trait1 Magic
//! Auto: ...simulator script...
//! Text [A] When this card attacks, ...
//! EndCard
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use super::effects::CommonEffects;
use crate::codes::{ImageDir, resolve_card_image};
use crate::model::{Card, CardType, Color, Trigger};
use crate::text::decode_text;

const MAX_TRAITS: usize = 3;

/// Card under construction; reset after every `EndCard`.
struct Draft {
    key: String,
    card_type: CardType,
    name: String,
    color: Color,
    level: u32,
    cost: u32,
    power: i32,
    triggers: [Option<Trigger>; 2],
    soul_count: u32,
    code: String,
    text: String,
    traits: Vec<String>,
    /// `Image` line: artwork file stem when it doesn't follow the card code.
    image: Option<String>,
    parsing_code: bool,
    parsing_text: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            key: String::new(),
            card_type: CardType::Character,
            name: String::new(),
            color: Color::None,
            level: 0,
            cost: 0,
            power: 1,
            triggers: [None, None],
            soul_count: 1,
            code: String::new(),
            text: String::new(),
            traits: Vec::new(),
            image: None,
            parsing_code: false,
            parsing_text: false,
        }
    }
}

impl Draft {
    fn add_trait(&mut self, t: &str) {
        if !t.is_empty() && self.traits.len() < MAX_TRAITS {
            self.traits.push(t.to_string());
        }
    }

    fn clone_from_card(&mut self, c: &Card) {
        self.card_type = c.card_type;
        self.name = c.name.clone();
        self.color = c.color;
        self.level = c.level;
        self.cost = c.cost;
        self.power = c.power;
        self.triggers = [c.triggers.first().copied(), c.triggers.get(1).copied()];
        self.soul_count = c.soul_count;
        self.code = c.code.clone();
        self.text = c.text.clone();
        for t in &c.traits {
            self.add_trait(t);
        }
    }

    fn finish(self, set_images: &ImageDir, alternate_artwork: &Path) -> Card {
        Card {
            image: resolve_card_image(
                set_images,
                &self.key,
                self.image.as_deref(),
                alternate_artwork,
            ),
            soul_count: if self.card_type == CardType::Climax {
                0
            } else {
                self.soul_count
            },
            key: self.key,
            card_type: self.card_type,
            name: self.name,
            color: self.color,
            level: self.level,
            cost: self.cost,
            power: self.power,
            triggers: self.triggers.into_iter().flatten().collect(),
            traits: self.traits,
            code: self.code,
            text: self.text,
        }
    }
}

/// `*XxxClimax` marker -> (triggers, color). `2SoulClimax` keeps the color already set.
fn climax_marker(marker: &str) -> Option<([Option<Trigger>; 2], Option<Color>)> {
    use Trigger::*;
    let soul2 = [Some(Soul), Some(Soul)];
    Some(match marker {
        "DoorClimax" => ([Some(Salvage), None], Some(Color::Red)),
        "StandbyClimax" => ([Some(Standby), Some(Soul)], Some(Color::Red)),
        "BookClimax" => ([Some(Book), None], Some(Color::Blue)),
        "GateClimax" => ([Some(Pant), Some(Soul)], Some(Color::Blue)),
        "BarClimax" => ([Some(Bar), None], Some(Color::Green)),
        "BagClimax" => ([Some(Bag), None], Some(Color::Green)),
        "WindClimax" => ([Some(Wind), None], Some(Color::Yellow)),
        "ChoiceClimax" => ([Some(Choice), None], Some(Color::Yellow)),
        "ShotClimax" => ([Some(Burn), Some(Soul)], Some(Color::Yellow)),
        "BlueStockSoul" | "2K2Climax(Blue)" | "3K1Climax(Blue)" => (soul2, Some(Color::Blue)),
        "YellowStockSoul" | "2K2Climax(Yellow)" | "3K1Climax(Yellow)" => {
            (soul2, Some(Color::Yellow))
        }
        "RedStockSoul" | "2K2Climax(Red)" | "3K1Climax(Red)" => (soul2, Some(Color::Red)),
        "GreenStockSoul" | "2K2Climax(Green)" | "3K1Climax(Green)" => (soul2, Some(Color::Green)),
        "2SoulClimax" => (soul2, None),
        _ => return None,
    })
}

/// Everything from the first `(` (or the whole string when there is none) is the parameter list.
fn split_params(s: &str) -> (&str, Option<Vec<&str>>) {
    match s.find('(') {
        Some(i) => {
            let (prefix, params) = s.split_at(i);
            let params = params.trim_start_matches('(').trim_end_matches(')');
            (prefix, Some(params.split(',').map(str::trim).collect()))
        }
        None => (s, None),
    }
}

/// Expands `*Effect(args)` using the common effects file; falls back to the raw reference.
fn expand_effect(reference: &str, effects: &CommonEffects) -> String {
    let parsed = reference.replace("()", "");
    if let Some(text) = effects.get(&parsed) {
        return text.trim().to_string();
    }

    let (ref_prefix, ref_params) = split_params(&parsed);
    if let Some(ref_params) = ref_params {
        for (effect_key, effect_text) in effects {
            let (key_prefix, key_params) = split_params(effect_key.trim());
            let Some(key_params) = key_params else {
                continue;
            };
            // The original gave up (and dropped the effect) on a parameter count mismatch;
            // keep looking for an overload with the right arity instead.
            if key_prefix != ref_prefix || key_params.len() != ref_params.len() {
                continue;
            }
            let mut text = effect_text.trim().to_string();
            for (name, value) in key_params.iter().zip(&ref_params) {
                if !name.is_empty() {
                    text = text.replace(name, value);
                }
            }
            return text
                .replace('$', "")
                .replace('|', " or ")
                .trim()
                .to_string();
        }
    }
    reference.trim().to_string()
}

pub fn parse_card_data(
    content: &str,
    set_path: &Path,
    alternate_artwork: &Path,
    effects: &CommonEffects,
) -> BTreeMap<String, Card> {
    let set_images = ImageDir::read(set_path);
    let (cards, forward_clones) =
        parse_pass(content, &set_images, alternate_artwork, effects, None);
    if !forward_clones {
        return cards;
    }
    // Some `Clone` pointed at a card defined further down: parse again, knowing every card.
    parse_pass(
        content,
        &set_images,
        alternate_artwork,
        effects,
        Some(&cards),
    )
    .0
}

/// One parse of a card file. `known` resolves clones of cards defined later in the file.
/// Also returns whether a clone source was missing.
fn parse_pass(
    content: &str,
    set_images: &ImageDir,
    alternate_artwork: &Path,
    effects: &CommonEffects,
    known: Option<&BTreeMap<String, Card>>,
) -> (BTreeMap<String, Card>, bool) {
    let mut missing_clone = false;
    let mut cards: BTreeMap<String, Card> = BTreeMap::new();
    let mut d = Draft::default();

    for line in content.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with("Combo") {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Character:") {
            d.key = card_key(rest);
            d.card_type = CardType::Character;
        } else if let Some(rest) = line.strip_prefix("Event:") {
            d.key = card_key(rest);
            d.card_type = CardType::Event;
        } else if let Some(rest) = line.strip_prefix("Climax:") {
            d.key = card_key(rest);
            d.card_type = CardType::Climax;
        } else if let Some(rest) = line.strip_prefix("Image ") {
            // Artwork is resolved at EndCard, from this or from the card key.
            d.image = Some(rest.trim().to_string()).filter(|i| !i.is_empty());
        } else if let Some(rest) = line.strip_prefix("Name ") {
            d.name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Color ") {
            d.color = match rest.trim() {
                "Y" => Color::Yellow,
                "B" => Color::Blue,
                "G" => Color::Green,
                "R" => Color::Red,
                _ => Color::None,
            };
        } else if let Some(rest) = line.strip_prefix("Level ") {
            parse_into(&mut d.level, rest, "level", &d.key);
        } else if let Some(rest) = line.strip_prefix("Clone ") {
            let source = rest.trim();
            match cards.get(source).or_else(|| known?.get(source)) {
                Some(c) => d.clone_from_card(c),
                None => missing_clone = true,
            }
        } else if let Some(rest) = line.strip_prefix("Cost ") {
            parse_into(&mut d.cost, rest, "cost", &d.key);
        } else if let Some(rest) = line.strip_prefix("Power ") {
            parse_into(&mut d.power, rest, "power", &d.key);
        } else if let Some(rest) = line.strip_prefix("Soul ") {
            parse_into(&mut d.soul_count, rest, "soul", &d.key);
        } else if let Some(rest) = line.strip_prefix("Trigger ") {
            if rest.trim() == "Soul" {
                d.triggers = [Some(Trigger::Soul), None];
            }
        } else if let Some(rest) = line.strip_prefix("Trait") {
            // `Trait1 Magic`: skip the trait number.
            let name = rest.get(1..).unwrap_or("").trim();
            d.add_trait(name);
        } else if let Some(rest) = line.strip_prefix('*') {
            if d.card_type == CardType::Climax {
                if let Some((triggers, color)) = climax_marker(rest.trim()) {
                    d.triggers = triggers;
                    if let Some(color) = color {
                        d.color = color;
                    }
                }
            } else {
                d.text.push_str(&expand_effect(rest, effects));
                d.text.push('\n');
            }
        } else if line.starts_with("EndCard") {
            let finished = std::mem::take(&mut d);
            // A stray `EndCard` (no card header before it) closes nothing.
            if !finished.key.is_empty() {
                let card = finished.finish(set_images, alternate_artwork);
                cards.insert(card.key.clone(), card);
            }
        } else if !line.starts_with("Text ")
            && (d.parsing_code
                || ["Cont:", "Auto:", "Act:", "Effect:"]
                    .iter()
                    .any(|p| line.starts_with(p)))
        {
            // Simulator script: everything until the next `Text` line.
            d.parsing_text = false;
            d.parsing_code = true;
            d.code.push_str(line.trim());
            d.code.push('\n');
        } else if let Some(rest) = line.strip_prefix("Text ") {
            d.parsing_code = false;
            d.parsing_text = true;
            // Texts may hold literal `\n` line breaks.
            d.text.push_str(&rest.trim().replace("\\n", "\n"));
            d.text.push('\n');
        } else if d.parsing_text {
            d.text.push_str(line.trim());
            d.text.push('\n');
        }
    }
    (cards, missing_clone)
}

/// `Event:: SMP/W60-096` (typo in the data) -> `SMP/W60-096`.
fn card_key(rest: &str) -> String {
    rest.trim_start_matches(':').trim().to_string()
}

fn parse_into<T: std::str::FromStr>(field: &mut T, raw: &str, what: &str, key: &str) {
    match raw.trim().parse() {
        Ok(v) => *field = v,
        Err(_) => eprintln!("{key}: could not parse {what} from {raw:?}"),
    }
}

/// Parses `<set_path>/CardData.txt`; a missing file yields an empty set.
/// Parses a set's card file: `<file>.txt` when the set names one, else `CardData.txt`
/// (file names are matched case-insensitively). A missing file yields an empty set.
pub fn load_cards(
    set_path: &Path,
    file: Option<&str>,
    alternate_artwork: &Path,
    effects: &CommonEffects,
) -> BTreeMap<String, Card> {
    let wanted = format!("{}.txt", file.unwrap_or("CardData")).to_lowercase();
    let path = std::fs::read_dir(set_path).ok().and_then(|entries| {
        entries.flatten().map(|e| e.path()).find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase() == wanted)
        })
    });
    match path.map(std::fs::read) {
        Some(Ok(bytes)) => {
            parse_card_data(&decode_text(&bytes), set_path, alternate_artwork, effects)
        }
        _ => BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::effects::parse_common_effects;

    const DATA: &str = "\
Character: KS/W49-E001
Image KS/W49-E001
Name Megumin
Color R
Level 3
Cost 2
Power 10000
Soul 2
Trigger Soul
Trait1 Magic
Trait2 Crimson Demon
Auto: script line 1
script line 2
Text [A] When this card attacks,
deal 1 damage.
*Brainstorm(2)
*Encore()
*UnknownEffect(1)
EndCard

Climax: KS/W49-E099
Name Explosion!
*BookClimax
Text [C] All your characters get +1000 power.
EndCard

Combo KS/W49-E001
Character: KS/W49-E001SP
Clone KS/W49-E001
Trait3 Foil
EndCard
";

    fn parse() -> BTreeMap<String, Card> {
        let effects = parse_common_effects(
            "Define: Brainstorm($X)\nText BRAINSTORM top $X cards|or not\n\
             Define: Encore\nText ENCORE pay 3\n",
        );
        let none = Path::new("/nonexistent");
        parse_card_data(DATA, none, none, &effects)
    }

    #[test]
    fn character() {
        let cards = parse();
        let c = &cards["KS/W49-E001"];
        assert_eq!(c.card_type, CardType::Character);
        assert_eq!(c.name, "Megumin");
        assert_eq!(c.color, Color::Red);
        assert_eq!((c.level, c.cost, c.power, c.soul_count), (3, 2, 10000, 2));
        assert_eq!(c.triggers, vec![Trigger::Soul]);
        assert_eq!(c.traits, vec!["Magic", "Crimson Demon"]);
        assert_eq!(c.code, "Auto: script line 1\nscript line 2\n");
        assert_eq!(
            c.text,
            "[A] When this card attacks,\ndeal 1 damage.\n\
             BRAINSTORM top 2 cards or or not\nENCORE pay 3\nUnknownEffect(1)\n"
        );
        assert_eq!(c.image, None);
    }

    #[test]
    fn climax() {
        let cards = parse();
        let c = &cards["KS/W49-E099"];
        assert_eq!(c.card_type, CardType::Climax);
        assert_eq!(c.triggers, vec![Trigger::Book]);
        assert_eq!(c.color, Color::Blue);
        assert_eq!(c.soul_count, 0);
        assert_eq!(c.power, 1);
    }

    #[test]
    fn clone_copies_and_extends() {
        let cards = parse();
        let c = &cards["KS/W49-E001SP"];
        assert_eq!(c.name, "Megumin");
        assert_eq!(c.level, 3);
        assert_eq!(c.traits, vec!["Magic", "Crimson Demon", "Foil"]);
        assert_eq!(c.text, cards["KS/W49-E001"].text);
    }

    #[test]
    fn new_format_events_and_typos() {
        let cards = parse_card_data(
            "Event: AYT/W110-027\nName Memory\nJPName 思い出\nRequirement StageContains 1\n\
             Text If you do not have a <Omiko City> character, this card cannot be played.\n\
             Effect:\n{\n    *Check4Add1(Omiko City)\n    Bounce\n}\n\
             Text Look at up to 4 cards.\\nYou may return it.\nEndCard\nEndCard\n\
             Event:: SMP/W60-096\nName Typo\nEndCard\n",
            Path::new("/nowhere"),
            Path::new("/nowhere"),
            &CommonEffects::new(),
        );
        assert_eq!(cards.len(), 2, "a stray EndCard adds no card");
        let e = &cards["AYT/W110-027"];
        assert_eq!(
            e.text,
            "If you do not have a <Omiko City> character, this card cannot be played.\n\
             Look at up to 4 cards.\nYou may return it.\n"
        );
        assert!(e.code.contains("Bounce"));
        assert_eq!(cards["SMP/W60-096"].name, "Typo");

        // A clone of a card defined further down the file.
        let cards = parse_card_data(
            "Character: A-002\nClone A-001\nTrait1 Foil\nEndCard\n\
             Character: A-001\nName Source\nLevel 2\nTrait1 Magic\nEndCard\n",
            Path::new("/nowhere"),
            Path::new("/nowhere"),
            &CommonEffects::new(),
        );
        assert_eq!(cards["A-002"].name, "Source");
        assert_eq!(cards["A-002"].level, 2);
        assert_eq!(cards["A-002"].traits, ["Magic", "Foil"]);
    }
}
