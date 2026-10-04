//! Renders every screen offscreen, on a fake simulator or, read-only, on a real install:
//! `SCREENSHOT_DIR=/some/dir [SIMULATOR_ROOT=~/Games/SimuWeiss] cargo test screenshots -- --ignored`
//! (needs a GPU or software Vulkan).

use std::fs;
use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use simu_deck_parser::decks;
use simu_deck_parser::settings::{SIMULATOR_PATH, Settings};

use super::{App, Screen};

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn card_art(path: &Path, rgb: [u8; 3]) {
    let img = image::RgbImage::from_fn(224, 313, |x, y| {
        let shade = 1.0 - (y as f32 / 313.0) * 0.5 + (x as f32 / 224.0) * 0.1;
        image::Rgb(rgb.map(|c| (c as f32 * shade).min(255.0) as u8))
    });
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    img.save(path).unwrap();
}

fn fake_simulator(root: &Path) -> Vec<String> {
    write(
        &root.join("Weiss Schwarz_Data/StreamingAssets/CommonEffects_USER_REFERENCE.txt"),
        "Define: Encore\n\tText [A] ENCORE [(3)]\n",
    );
    let cards = root.join("CardContent/Cards");
    let mut data = String::new();
    let mut codes = Vec::new();
    let colors = [
        ("R", [200, 60, 60]),
        ("B", [60, 90, 200]),
        ("Y", [210, 170, 40]),
        ("G", [60, 160, 80]),
    ];
    let traits = [
        "Magic",
        "Adventurer",
        "Goddess",
        "Noble",
        "Crusader",
        "Water",
        "Explosion",
    ];
    for i in 1..=40 {
        let (color, rgb) = colors[i % 4];
        let key = format!("KS/W49-E{i:03}");
        let kind = if i > 36 {
            "Climax"
        } else if i > 32 {
            "Event"
        } else {
            "Character"
        };
        data += &format!(
            "{kind}: {key}\nName Card number {i}\nColor {color}\nLevel {}\nCost {}\nPower {}\n\
             Trait1 {}\nTrait2 {}\nText Auto: [<sprite name=\"1cost\"/>] When this card becomes <sprite name=\"reverse\"/>, look at up to 2 cards from the top of your deck.\n\
             Text Cont: <b>ASSIST</b> All your other <Magic> characters get +500 power. <color=red>Once per turn.</color>\n\
             Text Auto: CxCombo When <sprite name=\"mini_door\"/> is placed on your climax area, this card gets +1000 power.\n\
             Text Cont: If \"Card number {}\" is on your stage, this card gets +1000 power.\n*Encore\n{}EndCard\n",
            i % 4,
            i % 3,
            1000 * (i % 9) + 500,
            traits[i % traits.len()],
            traits[(i + 3) % traits.len()],
            i % 40 + 1,
            if kind == "Climax" {
                "*GateClimax\n"
            } else {
                ""
            },
        );
        card_art(&cards.join(format!("Konosuba/W49/KS_W49_E{i:03}.jpg")), rgb);
        codes.push(key);
    }
    write(&cards.join("Konosuba/W49/CardData.txt"), &data);
    write(
        &cards.join("Konosuba/SingleSetData.txt"),
        "Series: KonoSuba!\nFolder: Konosuba/W49\nPrefix: KS/W49\nSetName: Konosuba (W49)\n",
    );
    for serie in ["Bang Dream", "Hololive", "Re Zero"] {
        write(
            &cards.join(format!("{serie}/SingleSetData.txt")),
            &format!("Series: {serie}\nFolder: Konosuba/W49\nPrefix: KS/W49\nSetName: Other\n"),
        );
    }
    codes
}

#[test]
#[ignore = "renders with wgpu; run manually"]
fn screenshots() {
    let out = std::env::var("SCREENSHOT_DIR").unwrap_or_else(|_| "target/screenshots".into());
    fs::create_dir_all(&out).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let real = std::env::var("SIMULATOR_ROOT").ok();
    let sim = match &real {
        Some(root) => Path::new(root).to_path_buf(),
        None => {
            let sim = dir.path().join("SimuWeiss");
            let codes = fake_simulator(&sim);
            let mut deck = Vec::new();
            for (i, code) in codes.iter().enumerate().take(14) {
                for _ in 0..(if i < 8 { 4 } else { 2 }) {
                    deck.push(code.clone());
                }
            }
            deck.push("XX/Y1-001".into());
            for name in [
                "Aqua burn",
                "Megumin explosion",
                "Standby control",
                "AI_Axis",
            ] {
                decks::save_new_deck(
                    &sim.join("Decks"),
                    name,
                    "10:00  01/01/2025",
                    "",
                    &deck,
                )
                .unwrap();
            }
            sim
        }
    };
    // Labels clicked in the walkthrough.
    let fake = real.is_none();
    let (chip, deck_name) = match real {
        Some(_) => ("Music", "Konosuba Aqua and Eris"),
        None => ("Magic", "Megumin explosion"),
    };

    for dark in [false, true] {
        let settings_path = dir.path().join(format!("settings-{dark}.conf"));
        let settings = Settings::at(&settings_path);
        settings
            .set(SIMULATOR_PATH, &sim.to_string_lossy())
            .unwrap();
        settings
            .set("ui_theme", if dark { "dark" } else { "light" })
            .unwrap();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1500.0, 950.0))
            .with_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            })
            .wgpu()
            .build_eframe(move |cc| App::with_settings(cc, Settings::at(&settings_path)));
        let theme = if dark { "dark" } else { "light" };
        let shot = |harness: &mut Harness<'_, App>, name: &str| {
            for _ in 0..40 {
                harness.step();
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            harness
                .render()
                .unwrap()
                .save(format!("{out}/{theme}-{name}.png"))
                .unwrap();
        };
        shot(&mut harness, "0-loading");
        for _ in 0..500 {
            harness.step();
            if harness.state().screen != Screen::Setup {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        shot(&mut harness, "1-cards");
        harness.hover_at(egui::pos2(620.0, 200.0));
        harness.step();
        harness.drag_at(egui::pos2(620.0, 200.0));
        harness.drop_at(egui::pos2(620.0, 200.0));
        if let Some(node) = harness.query_by_label(chip) {
            node.click();
        }
        shot(&mut harness, "2-cards-filtered");
        harness.get_by_label("List view").click();
        harness.hover_at(egui::pos2(350.0, 31.0));
        harness.drag_at(egui::pos2(350.0, 31.0));
        harness.drop_at(egui::pos2(350.0, 31.0));
        harness.run_steps(2);
        harness.event(egui::Event::Text("re".into()));
        shot(&mut harness, "3a-serie-picker");
        harness.key_press(egui::Key::Escape);
        harness.state_mut().notify(super::Notice::Error(
            "Could not load the deck".into(),
            "No such file or directory (os error 2)".into(),
        ));
        shot(&mut harness, "3b-error");
        harness.key_press(egui::Key::Escape);
        shot(&mut harness, "3-cards-list");
        // Quoted card names open a panel with their card, which stays open while the pointer
        // is in it.
        if let Some(link) = harness.query_all_by_label_contains("\"Card number").next() {
            link.hover();
        }
        shot(&mut harness, "3c-card-reference");
        if let Some(related) = harness.query_by_label("Related cards") {
            related.hover();
            shot(&mut harness, "3d-card-reference-panel");
            harness.get_by_label("Related cards").click();
            harness.remove_cursor();
            shot(&mut harness, "3e-related-cards");
            // A related card opens in the detail pane, shown in list view while pinned.
            harness.get_by_label("KS/W49-E006").click();
            harness.remove_cursor();
            shot(&mut harness, "3f-related-pinned");
            harness.get_by_label("Close window").click();
            harness.key_press(egui::Key::Escape);
            harness.run_steps(2);
        }
        harness.get_by_label("Decks").click();
        shot(&mut harness, "4-decks-empty");
        harness.get_by_label(deck_name).click();
        harness.hover_at(egui::pos2(480.0, 520.0));
        shot(&mut harness, "5-deck");
        // Upload confirmation only: uploading would publish a deck.
        harness.get_by_label("Upload").click();
        shot(&mut harness, "5-deck-upload");
        harness.get_by_label("Cancel").click();
        harness.run_steps(2);
        // Same with a linked account (a fake session: nothing is sent).
        harness.state_mut().account = Some(simu_deck_parser::encore::Account {
            name: "Megumin".into(),
            session: zeroize::Zeroizing::new("fake".into()),
        });
        harness.get_by_label("Upload").click();
        shot(&mut harness, "5-deck-upload-account");
        harness.get_by_label("Cancel").click();
        harness.run_steps(2);
        harness.state_mut().account = None;
        // PDF options only: saving would write into the real Downloads folder.
        for (button, name) in [
            ("Proxies", "5-deck-proxies"),
            ("Translation", "5-deck-translation"),
        ] {
            harness.get_by_label(button).click();
            shot(&mut harness, name);
            harness.get_by_label("Cancel").click();
            harness.run_steps(2);
        }

        // Deck builder: edit the open deck, add and remove cards, then cancel.
        let click = |harness: &mut Harness<'_, App>, pos: egui::Pos2, secondary: bool| {
            harness.hover_at(pos);
            harness.run_steps(2);
            let button = if secondary {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            };
            for pressed in [true, false] {
                harness.event(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
                harness.run_steps(2);
            }
        };
        harness.get_by_label("Edit").click();
        harness.run_steps(4);
        for pos in [(460.0, 200.0), (460.0, 200.0), (780.0, 420.0)] {
            click(&mut harness, egui::pos2(pos.0, pos.1), false);
        }
        click(&mut harness, egui::pos2(620.0, 200.0), true);
        harness.hover_at(egui::pos2(940.0, 420.0));
        shot(&mut harness, "5a-builder-edit");
        harness.get_by_label("Deck as a grid").click();
        harness.remove_cursor();
        shot(&mut harness, "5a-builder-grid");
        harness.get_by_label("Deck as a list").click();
        harness.get_by_label("Cancel").click();
        shot(&mut harness, "5b-discard");
        if let Some(discard) = harness.query_by_label("Discard") {
            discard.click();
        }
        harness.run_steps(4);

        // New deck, saved (only into the fake simulator).
        harness.get_by_label("New deck").click();
        harness.run_steps(4);
        // The name field has the focus when a new deck opens.
        harness.event(egui::Event::Text("My new deck".into()));
        harness.run_steps(2);
        for pos in [
            (460.0, 200.0),
            (460.0, 200.0),
            (620.0, 200.0),
            (780.0, 640.0),
        ] {
            click(&mut harness, egui::pos2(pos.0, pos.1), false);
        }
        harness.remove_cursor();
        shot(&mut harness, "5c-builder-new");
        if fake {
            harness.get_by_label("Save").click();
            shot(&mut harness, "5d-saved");
        } else {
            harness.get_by_label("Cancel").click();
            harness.run_steps(2);
            if let Some(discard) = harness.query_by_label("Discard") {
                discard.click();
            }
            harness.run_steps(2);
        }
        harness.get_by_label("Import").click();
        shot(&mut harness, "6-import");
        harness.key_press(egui::Key::Escape);
        // The Encore screen: the linked account's decks, previewed then imported.
        let account = simu_deck_parser::encore::Account {
            name: "Megumin".into(),
            session: zeroize::Zeroizing::new("fake".into()),
        };
        harness.state_mut().account = Some(account.clone());
        // Loaded before the screen opens: it would fetch the page over the network.
        harness.state_mut().encore_page.open_loaded(
            simu_deck_parser::encore::DeckPage {
                page: 1,
                total_pages: 2,
                total_decks: 30,
                decks: [
                    ("Megumin explosion", "abc123", ""),
                    ("Encore deck", "def456", "Tournament deck"),
                    ("Unnamed", "ghi789", ""),
                ]
                .into_iter()
                .map(|(name, id, description)| simu_deck_parser::encore::AccountDeck {
                    name: name.into(),
                    id: id.into(),
                    date: "03:04  01/02/2025".into(),
                    description: description.into(),
                })
                .collect(),
            },
        );
        harness.get_by_label("Encore").click();
        harness.run_steps(2);
        shot(&mut harness, "6b-encore");
        // A picked deck previews its cards (already resolved, so nothing is fetched).
        let series = harness.state().series.clone();
        let codes: Vec<String> = [
            "KS/W49-E001",
            "KS/W49-E001",
            "KS/W49-E002",
            "KS/W49-E033",
            "KS/W49-E037",
            "XX/Y1-001",
        ]
        .map(String::from)
        .to_vec();
        let resolved = simu_deck_parser::encore::build_deck(
            &simu_deck_parser::codes::CardIndex::new(&series),
            "Encore deck",
            "03:04  01/02/2025",
            &codes,
        );
        harness.state_mut().encore_page.preview_loaded(
            simu_deck_parser::encore::AccountDeck {
                name: "Encore deck".into(),
                id: "def456".into(),
                date: "03:04  01/02/2025".into(),
                description: "Tournament deck".into(),
            },
            resolved,
        );
        shot(&mut harness, "6c-encore-preview");
        harness.state_mut().account = None;
        harness.run_steps(2);
        harness.get_by_label("Settings").click();
        shot(&mut harness, "7-settings");
        harness.state_mut().account = Some(simu_deck_parser::encore::Account {
            name: "Megumin".into(),
            session: zeroize::Zeroizing::new("fake".into()),
        });
        shot(&mut harness, "7-settings-account");
        harness.state_mut().account = None;
        harness.get_by_label("Change folder…").click();
        shot(&mut harness, "8-setup");
        // Empty field: shows the simulators detected on this computer.
        harness.state_mut().setup.set_path(String::new());
        for _ in 0..300 {
            harness.step();
            if harness.state().setup.detecting.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        shot(&mut harness, "9-setup-detected");
    }
}




