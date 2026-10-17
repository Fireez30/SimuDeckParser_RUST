//! The card list's filters (type, colour, level… and the search field) and the two sort orders
//! cards are shown in. Independent from the UI, so it is unit-tested.

use std::collections::BTreeSet;

use crate::model::{Card, CardType, Color};
use crate::text::contains_ignore_case;

/// Sorts cards as the card list shows them: by colour, then type, cost, power and code.
pub fn sort_for_card_list(cards: &mut [Card]) {
    cards.sort_by(|a, b| {
        // Tuples compare field by field: the first difference decides.
        (a.color, a.card_type, a.cost, a.power, &a.key, a.level).cmp(&(
            b.color,
            b.card_type,
            b.cost,
            b.power,
            &b.key,
            b.level,
        ))
    });
}

/// Sorts cards as decks show them: characters, then events, then climaxes; inside each type
/// by level, cost, power and colour.
pub fn sort_for_deck(cards: &mut [Card]) {
    cards.sort_by(|a, b| {
        (a.card_type, a.level, a.cost, a.power, a.color).cmp(&(
            b.card_type,
            b.level,
            b.cost,
            b.power,
            b.color,
        ))
    });
}

/// One group of filter chips: every value present in the loaded cards, and the selected ones.
/// `T` is the type of the values (levels are numbers, colours are [`Color`]s…); `T: Ord + Clone`
/// says it must be sortable and copyable.
#[derive(Debug, Clone)]
pub struct Choice<T: Ord + Clone> {
    pub available: Vec<T>,
    pub selected: BTreeSet<T>,
}

impl<T: Ord + Clone> Default for Choice<T> {
    fn default() -> Self {
        Self {
            available: Vec::new(),
            selected: BTreeSet::new(),
        }
    }
}

impl<T: Ord + Clone> Choice<T> {
    fn from_values(values: impl IntoIterator<Item = T>) -> Self {
        let set: BTreeSet<T> = values.into_iter().collect();
        Self {
            available: set.iter().cloned().collect(),
            selected: set,
        }
    }

    pub fn contains(&self, v: &T) -> bool {
        self.selected.contains(v)
    }

    pub fn toggle(&mut self, v: &T, on: bool) {
        if on {
            self.selected.insert(v.clone());
        } else {
            self.selected.remove(v);
        }
    }

    /// Every available value is selected, i.e. this group filters nothing out.
    pub fn is_all(&self) -> bool {
        self.available.iter().all(|v| self.selected.contains(v))
    }

    pub fn select_all(&mut self) {
        self.selected = self.available.iter().cloned().collect();
    }

    /// Whether `v` shows as picked in a chip UI: nothing is picked while the group is unfiltered.
    pub fn is_picked(&self, v: &T) -> bool {
        !self.is_all() && self.contains(v)
    }

    /// Chip click: the first pick narrows the group to `v`, later picks add or remove values,
    /// and removing the last pick clears the filter.
    pub fn click(&mut self, v: &T) {
        if self.is_all() {
            self.selected = [v.clone()].into();
        } else if !self.selected.remove(v) {
            self.selected.insert(v.clone());
        }
        if self.selected.is_empty() {
            self.select_all();
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub levels: Choice<u32>,
    pub costs: Choice<u32>,
    pub colors: Choice<Color>,
    pub types: Choice<CardType>,
    pub traits: Choice<String>,
    /// Some loaded card has the climax combo keyword, so the filter is offered.
    pub has_cx_combo: bool,
    /// Only show cards with the climax combo keyword.
    pub cx_combo: bool,
    /// Name / code / text search, applied on top of the other filters.
    pub search: String,
}

impl Filters {
    /// No checkbox group filters anything out (the search is not considered).
    pub fn is_unfiltered(&self) -> bool {
        self.levels.is_all()
            && self.costs.is_all()
            && self.colors.is_all()
            && self.types.is_all()
            && self.traits.is_all()
            && !self.cx_combo
    }

    pub fn clear(&mut self) {
        self.levels.select_all();
        self.costs.select_all();
        self.colors.select_all();
        self.types.select_all();
        self.traits.select_all();
        self.cx_combo = false;
    }

    /// Offers every value present in `cards`, all selected (nothing filtered out yet).
    pub fn from_cards(cards: &[Card]) -> Self {
        Self {
            levels: Choice::from_values(cards.iter().map(|c| c.level)),
            costs: Choice::from_values(cards.iter().map(|c| c.cost)),
            colors: Choice::from_values(cards.iter().map(|c| c.color)),
            types: Choice::from_values(cards.iter().map(|c| c.card_type)),
            traits: Choice::from_values(cards.iter().flat_map(|c| c.traits.iter().cloned())),
            has_cx_combo: cards.iter().any(is_cx_combo),
            cx_combo: false,
            search: String::new(),
        }
    }

    pub fn matches(&self, c: &Card) -> bool {
        let search = self.search.trim();
        let found = search.is_empty()
            || contains_ignore_case(search, &c.name)
            || contains_ignore_case(search, &c.key)
            || contains_ignore_case(search, &c.text);
        found
            && self.levels.contains(&c.level)
            && self.costs.contains(&c.cost)
            && self.colors.contains(&c.color)
            && self.types.contains(&c.card_type)
            // Traitless cards (climaxes, most events) only drop out once traits are narrowed.
            && (self.traits.is_all() || c.traits.iter().any(|t| self.traits.contains(t)))
            && (!self.cx_combo || is_cx_combo(c))
    }

    /// The cards that pass every filter and the search.
    pub fn apply(&self, cards: &[Card]) -> Vec<Card> {
        cards.iter().filter(|c| self.matches(c)).cloned().collect()
    }
}

/// The card text has the climax combo keyword.
pub fn is_cx_combo(c: &Card) -> bool {
    c.text.contains("CxCombo")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Trigger;

    fn card(key: &str, t: CardType, color: Color, level: u32, cost: u32, power: i32) -> Card {
        Card {
            key: key.into(),
            card_type: t,
            name: format!("name {key}"),
            image: None,
            color,
            level,
            cost,
            power,
            triggers: vec![Trigger::Soul],
            soul_count: 1,
            traits: vec![],
            code: String::new(),
            text: String::new(),
        }
    }

    fn keys(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.key.as_str()).collect()
    }

    #[test]
    fn deck_sort() {
        use CardType::*;
        let mut cards = vec![
            card("cx", Climax, Color::Red, 0, 0, 0),
            card("l3", Character, Color::Red, 3, 2, 10000),
            card("l0b", Character, Color::Blue, 0, 0, 3000),
            card("l0a", Character, Color::Yellow, 0, 0, 3000),
            card("ev", Event, Color::Red, 1, 1, 0),
            card("l0c", Character, Color::Red, 0, 0, 1500),
        ];
        sort_for_deck(&mut cards);
        assert_eq!(keys(&cards), vec!["l0c", "l0a", "l0b", "l3", "ev", "cx"]);

        sort_for_card_list(&mut cards);
        assert_eq!(keys(&cards), vec!["l0a", "l0b", "l0c", "l3", "ev", "cx"]);
    }

    #[test]
    fn card_list_sort() {
        use CardType::*;
        let mut cards = vec![
            card("b-cx", Climax, Color::Blue, 0, 0, 0),
            card("r-ev", Event, Color::Red, 1, 0, 0),
            card("r-c1-9k", Character, Color::Red, 2, 1, 9000),
            card("r-c1-6k", Character, Color::Red, 2, 1, 6000),
            card("r-c0", Character, Color::Red, 3, 0, 10000),
            card("y-c2", Character, Color::Yellow, 3, 2, 10000),
            card("b-c0", Character, Color::Blue, 0, 0, 3000),
        ];
        sort_for_card_list(&mut cards);
        assert_eq!(
            keys(&cards),
            vec!["y-c2", "b-c0", "b-cx", "r-c0", "r-c1-6k", "r-c1-9k", "r-ev"]
        );
    }

    #[test]
    fn filtering() {
        let mut a = card("KS/W49-001", CardType::Character, Color::Red, 0, 0, 1);
        a.traits = vec!["Magic".into()];
        let mut b = card("KS/W49-002", CardType::Character, Color::Blue, 1, 0, 1);
        b.traits = vec!["Water".into()];
        b.text = "Brainstorm".into();
        let cards = [a, b];

        let mut f = Filters::from_cards(&cards);
        assert_eq!(f.levels.available, vec![0, 1]);
        assert_eq!(f.traits.available, vec!["Magic", "Water"]);
        assert_eq!(f.apply(&cards).len(), 2);

        f.toggle_trait_off("Magic");
        assert_eq!(keys(&f.apply(&cards)), vec!["KS/W49-002"]);

        f.colors.toggle(&Color::Blue, false);
        assert!(f.apply(&cards).is_empty());

        // The search applies on top of the filters.
        f.search = "brainSTORM".into();
        assert!(f.apply(&cards).is_empty());
        f.clear();
        assert_eq!(keys(&f.apply(&cards)), vec!["KS/W49-002"]);
        f.search = "w49-001".into();
        assert_eq!(keys(&f.apply(&cards)), vec!["KS/W49-001"]);
        f.colors.toggle(&Color::Red, false);
        assert!(f.apply(&cards).is_empty());
    }

    impl Filters {
        fn toggle_trait_off(&mut self, t: &str) {
            self.traits.toggle(&t.to_string(), false);
        }
    }

    #[test]
    fn traitless_cards_only_hidden_when_traits_are_narrowed() {
        let mut a = card("a", CardType::Character, Color::Red, 0, 0, 0);
        a.traits = vec!["Magic".into()];
        let mut b = card("b", CardType::Character, Color::Red, 0, 0, 0);
        b.traits = vec!["Water".into()];
        let cards = [a, b, card("cx", CardType::Climax, Color::Red, 0, 0, 0)];
        let mut f = Filters::from_cards(&cards);
        assert_eq!(f.apply(&cards).len(), 3);
        f.traits.click(&"Magic".to_string());
        assert_eq!(keys(&f.apply(&cards)), vec!["a"]);
    }

    #[test]
    fn cx_combo_filter() {
        let mut a = card("a", CardType::Character, Color::Red, 0, 0, 0);
        a.text = "Auto: CxCombo When a climax is placed".into();
        let cards = [a, card("b", CardType::Character, Color::Red, 0, 0, 0)];
        let mut f = Filters::from_cards(&cards);
        assert!(f.has_cx_combo && f.is_unfiltered());
        f.cx_combo = true;
        assert!(!f.is_unfiltered());
        assert_eq!(keys(&f.apply(&cards)), vec!["a"]);
        f.clear();
        assert_eq!(f.apply(&cards).len(), 2);
        assert!(!Filters::from_cards(&cards[1..]).has_cx_combo);
    }

    #[test]
    fn chip_clicks_narrow_then_widen() {
        let mut c = Choice::from_values([1, 2, 3]);
        assert!(c.is_all() && !c.is_picked(&1));
        c.click(&2);
        assert_eq!(c.selected, [2].into());
        assert!(c.is_picked(&2) && !c.is_picked(&1));
        c.click(&3);
        assert_eq!(c.selected, [2, 3].into());
        c.click(&2);
        c.click(&3);
        assert!(c.is_all());
    }
}
