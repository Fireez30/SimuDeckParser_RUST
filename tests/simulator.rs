//! End-to-end on a fake simulator folder: parse cards, then save and load decks.

use std::fs;
use std::path::Path;

use simu_deck_parser::codes::{CardIndex, transform_to_existing_keys};
use simu_deck_parser::decks;
use simu_deck_parser::encore::build_deck;
use simu_deck_parser::filters::{DECK_ORDER, sort_cards};
use simu_deck_parser::layout::Layout;
use simu_deck_parser::model::{CardType, Trigger};
use simu_deck_parser::parser::load_simulator;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fake_simulator(root: &Path) {
    write(
        &root.join("Weiss Schwarz_Data/StreamingAssets/CommonEffects_USER_REFERENCE.txt"),
        "\u{feff}//comment\nDefine: Encore\n\tTag Encore\n\tText [A] ENCORE [(3)]\n",
    );
    let cards = root.join("CardContent/Cards");
    write(
        &cards.join("Konosuba/SingleSetData.txt"),
        "Series: KonoSuba\nFolder: Konosuba/W49\nPrefix: KS/W49\nSetName: Konosuba\n",
    );
    write(
        &cards.join("Konosuba/W49/CardData.txt"),
        "Character: KS/W49-E001\nName Aqua\nColor B\nLevel 1\nPower 5000\nTrait1 Goddess\n*Encore\nEndCard\n\
         Climax: KS/W49-E099\nName Splash\n*GateClimax\nEndCard\n\
         Character: KS/W49-E002\nName Megumin\nColor R\nLevel 0\nPower 2500\nEndCard\n",
    );
    write(&cards.join("Konosuba/W49/KS_W49_E001.jpg"), "");
}

#[test]
fn load_and_import_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    fake_simulator(dir.path());
    let layout = Layout::find(&dir.path().join("CardContent")).unwrap();

    let series = load_simulator(&layout).unwrap();
    let serie = series.iter().find(|s| s.name == "KonoSuba").unwrap();
    let cards = &serie.sets[0].cards;
    assert_eq!(cards.len(), 3);
    assert_eq!(cards["KS/W49-E001"].text, "[A] ENCORE [(3)]\n");
    assert!(cards["KS/W49-E001"].image.is_some());
    assert_eq!(
        cards["KS/W49-E099"].triggers,
        vec![Trigger::Pant, Trigger::Soul]
    );
    assert_eq!(cards["KS/W49-E002"].image, None);

    // Encore Decks gives JP-style codes; they resolve to the simulator's EN keys.
    let index = CardIndex::new(&series);
    let imported: Vec<String> = ["KS/W49-001", "KS/W49-099", "KS/W49-002SP", "XX/Y1-001"]
        .map(String::from)
        .to_vec();
    let codes = transform_to_existing_keys(&index.codes(), &imported);
    assert_eq!(
        codes,
        ["KS/W49-E001", "KS/W49-E099", "KS/W49-E002", "XX/Y1-001"]
    );

    decks::save_new_deck(&layout.decks(), "Test", "10:00  01/01/2025", "", &codes).unwrap();
    let stored = decks::list_decks(&layout.decks()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].codes, codes);

    let (mut deck, missing) = build_deck(&index, "Test", "", &stored[0].codes);
    assert_eq!(missing, ["XX/Y1-001"]);
    sort_cards(&mut deck.cards, DECK_ORDER);
    let order: Vec<(&str, CardType)> = deck
        .cards
        .iter()
        .map(|c| (c.name.as_str(), c.card_type))
        .collect();
    assert_eq!(
        order,
        [
            ("Megumin", CardType::Character),
            ("Aqua", CardType::Character),
            ("Splash", CardType::Climax)
        ]
    );
}

/// Loads a real install: `SIMULATOR_ROOT=~/Games/SimuWeiss cargo test -- --ignored real_install --nocapture`.
#[test]
#[ignore = "needs a simulator install in SIMULATOR_ROOT"]
fn real_install() {
    let root = std::env::var("SIMULATOR_ROOT").expect("SIMULATOR_ROOT");
    let layout = Layout::find(Path::new(&root)).expect("not a simulator folder");
    let start = std::time::Instant::now();
    let series = load_simulator(&layout).unwrap();
    let all: Vec<_> = series
        .iter()
        .flat_map(|s| &s.sets)
        .flat_map(|s| s.cards.values())
        .collect();
    let without_image = all.iter().filter(|c| c.image.is_none()).count();
    let without_text = all
        .iter()
        .filter(|c| c.card_type != CardType::Climax && c.text.trim().is_empty())
        .count();
    let unexpanded = all.iter().filter(|c| c.text.contains('*')).count();
    println!(
        "{} series, {} sets, {} cards in {:?}; {without_image} without image, \
         {without_text} non-climax without text, {unexpanded} with a '*'",
        series.len(),
        series.iter().map(|s| s.sets.len()).sum::<usize>(),
        all.len(),
        start.elapsed(),
    );
    assert!(!series.is_empty());

    let index = CardIndex::new(&series);
    for deck in decks::list_decks(&layout.decks()).unwrap() {
        let (built, missing) = build_deck(&index, &deck.name, &deck.date, &deck.codes);
        println!(
            "deck {:?}{}: {} cards, sleeve {:?}, missing {:?}",
            deck.name,
            if deck.ai { " (AI)" } else { "" },
            built.cards.len(),
            deck.sleeve,
            missing
        );
    }
}

/// Lists cards without artwork or with unexpanded effects on a real install.
#[test]
#[ignore = "needs a simulator install in SIMULATOR_ROOT"]
fn real_install_gaps() {
    let root = std::env::var("SIMULATOR_ROOT").expect("SIMULATOR_ROOT");
    let layout = Layout::find(Path::new(&root)).unwrap();
    let series = load_simulator(&layout).unwrap();
    for set in series.iter().flat_map(|s| &s.sets) {
        let missing: Vec<_> = set.cards.values().filter(|c| c.image.is_none()).collect();
        if !missing.is_empty() {
            println!(
                "{} ({}): {}/{} without image, e.g. {}",
                set.name,
                set.path.display(),
                missing.len(),
                set.cards.len(),
                missing[0].key
            );
        }
    }
    for card in series
        .iter()
        .flat_map(|s| &s.sets)
        .flat_map(|s| s.cards.values())
        .filter(|c| c.text.contains('*'))
        .take(5)
    {
        println!("{}: {:?}", card.key, card.text);
    }
}

#[test]
#[ignore = "needs a simulator install in SIMULATOR_ROOT"]
fn real_install_text_gaps() {
    let root = std::env::var("SIMULATOR_ROOT").expect("SIMULATOR_ROOT");
    let series = load_simulator(&Layout::find(Path::new(&root)).unwrap()).unwrap();
    let cards: Vec<_> = series
        .iter()
        .flat_map(|s| &s.sets)
        .flat_map(|s| s.cards.values())
        .collect();
    for c in cards
        .iter()
        .filter(|c| c.card_type != CardType::Climax && c.text.trim().is_empty())
        .step_by(300)
    {
        println!(
            "NO TEXT {} name={:?} code={:?}",
            c.key,
            c.name,
            c.code.lines().next()
        );
    }
    for c in cards.iter().filter(|c| c.text.contains('*')).step_by(15) {
        println!("STAR {}: {:?}", c.key, c.text);
    }
}

/// Imports a live Encore Decks deck against a real install's cards, into a temporary folder:
/// `SIMULATOR_ROOT=… cargo test -- --ignored real_install_import --nocapture`.
#[test]
#[ignore = "needs SIMULATOR_ROOT and network access"]
fn real_install_import() {
    let root = std::env::var("SIMULATOR_ROOT").expect("SIMULATOR_ROOT");
    let series = load_simulator(&Layout::find(Path::new(&root)).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let link = "https://www.encoredecks.com/deck/PPyvcLuvt";
    let (deck, missing) = simu_deck_parser::encore::import(link, &series, dir.path()).unwrap();
    let saved = decks::list_decks(dir.path()).unwrap();
    println!(
        "imported {:?}: {} cards, missing {missing:?}\n{}",
        deck.name,
        deck.cards.len(),
        fs::read_to_string(&saved[0].path)
            .unwrap()
            .lines()
            .take(8)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].codes.len(), 50);
}
