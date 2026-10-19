//! The sleeve picker: a button showing the deck's sleeve, which opens a popup with every sleeve
//! of the simulator as thumbnails, a name filter, and a form to add a new sleeve from a file or
//! a link. Used by the deck editor and the deck page.

use std::path::PathBuf;

use egui::{CornerRadius, RichText, ScrollArea, Sense, Stroke, Ui, vec2};
use egui_material_icons::icons::ICON_ADD;
use simu_deck_parser::model::CARD_IMAGE_RATIO;
use simu_deck_parser::simulator::sleeves::{Sleeve, SleeveSource, add_sleeve};

use super::assets;
use super::background::{Task, finished};
use super::theme::{self, scheme};

/// Width the sleeve picker wraps its thumbnails at.
const SLEEVE_GRID_WIDTH: f32 = 480.0;
/// Columns of thumbnails in the sleeve picker.
const SLEEVE_GRID_COLUMNS: usize = 5;
/// Height the sleeve popup keeps when nothing is filtered.
const SLEEVE_POPUP_HEIGHT: f32 = 440.0;

/// The sleeves of the simulator as a picker: a name filter, then "No sleeve", then a
/// fixed-height grid of thumbnails the user scrolls. Sets `picked` to the chosen sleeve
/// file name, empty to have none. The filter text lives in egui memory at `id` until the
/// popup closes.
fn sleeve_grid(
    ui: &mut Ui,
    id: egui::Id,
    sleeves: &[Sleeve],
    current: &str,
    picked: &mut Option<String>,
) {
    // A width of its own, so the popup does not size itself to the (narrow) button.
    ui.set_width(SLEEVE_GRID_WIDTH);

    let query_id = id.with("sleeve_query");
    let mut query = ui
        .data(|d| d.get_temp::<String>(query_id))
        .unwrap_or_default();
    let edit = ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("Find a sleeve")
            .desired_width(f32::INFINITY),
    );
    // Typing goes to the filter, unless the user is in another field (adding a sleeve).
    if !edit.has_focus() && query.is_empty() && ui.memory(|m| m.focused().is_none()) {
        edit.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(query_id, query.clone()));

    if ui
        .selectable_label(current.trim().is_empty(), "No sleeve")
        .on_hover_text("The deck is played without a sleeve")
        .clicked()
    {
        *picked = Some(String::new());
        ui.close();
    }
    ui.separator();

    let needle = query.trim().to_lowercase();
    let shown: Vec<&Sleeve> = sleeves
        .iter()
        .filter(|sl| needle.is_empty() || sl.name.to_lowercase().contains(&needle))
        .collect();
    // A fixed region for the grid: a plain scroll area would fill all the height the
    // popup still has, and the popup would grow to the whole screen while the grid
    // stayed small. Fixed, the popup is always the same, healthy size.
    ui.allocate_ui(vec2(ui.available_width(), SLEEVE_POPUP_HEIGHT), |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            if shown.is_empty() {
                ui.label(
                    RichText::new(format!("No sleeve matches \"{}\"", query.trim()))
                        .color(scheme(ui).on_surface_variant),
                );
            } else {
                // Rows of fixed columns: `horizontal_wrapped` does not wrap inside a popup.
                egui::Grid::new(id.with("sleeve_grid"))
                    .num_columns(SLEEVE_GRID_COLUMNS)
                    .spacing(vec2(8.0, 8.0))
                    .show(ui, |ui| {
                        for (i, sleeve) in shown.iter().enumerate() {
                            let selected = sleeve.name.eq_ignore_ascii_case(current.trim());
                            sleeve_item(ui, sleeve, selected, picked);
                            if (i + 1) % SLEEVE_GRID_COLUMNS == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });
    });
}

/// Adding an image to the simulator's `Sleeves` folder from a path or a link, in the
/// sleeve popup. The copy or download runs on a background thread.
#[derive(Default)]
pub(super) struct SleeveAdder {
    /// The simulator's `Sleeves` folder, where the image is copied.
    pub dir: PathBuf,
    /// Path or link to the image, as typed.
    source: String,
    /// Name for the new sleeve; empty, the image's file name.
    name: String,
    task: Option<Task<Sleeve>>,
    error: Option<String>,
}

impl SleeveAdder {
    /// A form adding sleeves to `dir`, the simulator's `Sleeves` folder.
    pub(super) fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            ..Self::default()
        }
    }

    /// Sets `picked` to the new sleeve once it is in the folder.
    fn poll(&mut self, picked: &mut Option<String>) {
        match finished(&mut self.task) {
            Some(Ok(sleeve)) => {
                self.source.clear();
                self.name.clear();
                self.error = None;
                *picked = Some(sleeve.name);
            }
            Some(Err(e)) => self.error = Some(format!("{e:#}")),
            None => {}
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(source) = SleeveSource::parse(&self.source) else {
            return;
        };
        if self.dir.as_os_str().is_empty() {
            self.error = Some("The simulator folder is not set.".into());
            return;
        }
        self.error = None;
        let dir = self.dir.clone();
        let name = self.name.trim().to_string();
        self.task = Some(Task::spawn(ctx, move || add_sleeve(&dir, &source, &name)));
    }

    /// The fields and button under the sleeve grid.
    fn show(&mut self, ui: &mut Ui) {
        let s = scheme(ui);
        let running = self.task.is_some();
        ui.separator();
        ui.label(theme::label("Add a sleeve").color(s.on_surface_variant));
        let source = ui.add_enabled(
            !running,
            egui::TextEdit::singleline(&mut self.source)
                .hint_text("Image path or link (JPEG or PNG)")
                .desired_width(f32::INFINITY),
        );
        let valid = SleeveSource::parse(&self.source).is_some();
        ui.horizontal(|ui| {
            let button_width = 90.0;
            let name = ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text("Name (optional)")
                    .desired_width(ui.available_width() - button_width - 8.0),
            );
            let entered = (source.lost_focus() || name.lost_focus())
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if running {
                ui.spinner();
                ui.label(RichText::new("Adding…").color(s.on_surface_variant));
            } else {
                let add = theme::tonal(Some(ICON_ADD), "Add")
                    .enabled(valid)
                    .show(ui)
                    .on_hover_text("Copy the image into the simulator's Sleeves folder");
                if valid && (add.clicked() || entered) {
                    self.start(ui.ctx());
                }
            }
        });
        if let Some(error) = &self.error {
            ui.label(theme::body_small(error).color(s.error));
        }
    }
}

/// A combo box for the deck's sleeve: a button that opens the popup with
/// [`sleeve_grid`] and the `adder` of new sleeves. Sets `picked` to the
/// chosen sleeve file name, empty for none, or to the new sleeve once it is added.
///
/// Not an `egui::ComboBox`: its popup, once shrunk by a filter, never grows back,
/// while this one returns to its full height when the filter is cleared.
pub(super) fn sleeve_combo(
    ui: &mut Ui,
    id: egui::Id,
    width: f32,
    sleeves: &[Sleeve],
    current: &str,
    picked: &mut Option<String>,
    adder: &mut SleeveAdder,
) -> egui::Response {
    adder.poll(picked);
    let label = if sleeves.is_empty() {
        "No sleeves found".to_string()
    } else if current.trim().is_empty() {
        "None".to_string()
    } else {
        Sleeve::display_name(current.trim()).to_string()
    };
    let button = egui::Button::new(format!("{label} \u{25be}"))
        .wrap_mode(egui::TextWrapMode::Truncate)
        .min_size(vec2(width, 0.0));
    // Enabled even without sleeves: the popup is where the first one is added.
    let response = ui.add(button);
    let shown = egui::Popup::from_toggle_button_response(&response)
        .id(id.with("sleeve_popup"))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            sleeve_grid(ui, id, sleeves, current, picked);
            adder.show(ui);
        });
    if shown.is_none() {
        clear_sleeve_query(ui.ctx(), id);
    }
    response
}

/// Forgets the sleeve filter of the picker `id`: called when its popup closes.
fn clear_sleeve_query(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.remove_temp::<String>(id.with("sleeve_query")));
}

/// One sleeve of the picker: thumbnail and name, highlighted when selected.
fn sleeve_item(ui: &mut Ui, sleeve: &Sleeve, selected: bool, picked: &mut Option<String>) {
    let s = scheme(ui);
    let name = Sleeve::display_name(&sleeve.name);
    ui.vertical(|ui| {
        ui.set_max_width(80.0);
        let image = assets::image_from_path(&sleeve.path)
            .fit_to_exact_size(vec2(80.0, 80.0 * CARD_IMAGE_RATIO))
            .corner_radius(CornerRadius::same(6))
            .sense(Sense::click());
        let r = ui.add(image);
        if selected {
            ui.painter().rect_stroke(
                r.rect,
                CornerRadius::same(6),
                Stroke::new(2.0, s.primary),
                egui::StrokeKind::Outside,
            );
        }
        let r = r
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_ui(|ui| {
                ui.set_width(200.0);
                ui.add(
                    assets::image_from_path(&sleeve.path)
                        .fit_to_exact_size(vec2(160.0, 160.0 * CARD_IMAGE_RATIO)),
                );
            });
        if r.clicked() {
            *picked = Some(sleeve.name.clone());
            ui.close();
        }
        // The name is clickable too, not only the picture.
        let text = RichText::new(name).size(9.0).color(if selected {
            s.on_surface
        } else {
            s.on_surface_variant
        });
        if ui
            .add(egui::Button::new(text).frame(false))
            .on_hover_ui(|ui| {
                ui.set_width(200.0);
                ui.add(
                    assets::image_from_path(&sleeve.path)
                        .fit_to_exact_size(vec2(160.0, 160.0 * CARD_IMAGE_RATIO)),
                );
            })
            .clicked()
        {
            *picked = Some(sleeve.name.clone());
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::{NodeT, Queryable};

    fn fake_sleeves(n: usize) -> Vec<Sleeve> {
        (0..n)
            .map(|i| Sleeve {
                name: format!("Sleeve {i}.jpg"),
                path: format!("/nowhere/sleeve {i}.jpg").into(),
            })
            .collect()
    }

    /// The picker lays the sleeves out in wrapped rows, not one endless line, and picking
    /// one (or none at all) reports it.
    #[test]
    fn sleeve_grid_wraps_into_rows() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked = RefCell::new(None);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_grid"), &sleeves, "", &mut picked);
            });
        harness.run_steps(2);

        let first = harness.get_by_label("Sleeve 0").rect();
        let last = harness.get_by_label("Sleeve 19").rect();
        assert!(
            first.min.y < last.min.y,
            "the grid must have several rows, got one at y {} and {}",
            first.min.y,
            last.min.y
        );
        assert!(
            last.max.x < SLEEVE_GRID_WIDTH + 20.0,
            "the grid must wrap at {SLEEVE_GRID_WIDTH}, sleeve 19 ends at x {}",
            last.max.x
        );

        harness.get_by_label("Sleeve 19").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some("Sleeve 19.jpg"));

        harness.get_by_label("No sleeve").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some(""));
    }

    /// Typing in the picker keeps only the sleeves whose name matches.
    #[test]
    fn sleeve_grid_filters_by_name() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked = RefCell::new(None);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_filter"), &sleeves, "", &mut picked);
            });
        harness.run_steps(2);
        assert!(harness.query_by_label("Sleeve 2").is_some());
        assert!(harness.query_by_label("Sleeve 19").is_some());

        let filter = harness.get_by_role(egui::accesskit::Role::TextInput);
        filter.click();
        filter.type_text("19");
        harness.run_steps(2);
        assert!(harness.query_by_label("Sleeve 19").is_some());
        assert!(
            harness.query_by_label("Sleeve 2").is_none(),
            "a filtered-out sleeve must not be in the grid"
        );

        // Picking the only match works with the filter on.
        harness.get_by_label("Sleeve 19").click();
        harness.run_steps(2);
        assert_eq!(picked.borrow().as_deref(), Some("Sleeve 19.jpg"));
    }

    /// The picker keeps a bounded height: full size with the whole list, smaller with a
    /// filter, and back to full size when the filter is cleared. It must never grow to
    /// the screen (as it did when the scroll area filled whatever height was left).
    #[test]
    fn sleeve_grid_keeps_a_bounded_fixed_height() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked: RefCell<Option<String>> = RefCell::new(None);
        let height = RefCell::new(0.0);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                sleeve_grid(ui, egui::Id::new("test_height"), &sleeves, "", &mut picked);
                *height.borrow_mut() = ui.min_rect().height();
            });
        harness.run_steps(2);
        let unfiltered = *height.borrow();
        assert!(
            unfiltered >= SLEEVE_POPUP_HEIGHT,
            "the picker must be at least the grid's {SLEEVE_POPUP_HEIGHT}, got {unfiltered}"
        );
        assert!(
            unfiltered < 600.0,
            "the picker must stay bounded, got {unfiltered}"
        );

        let filter = harness.get_by_role(egui::accesskit::Role::TextInput);
        filter.click();
        filter.type_text("19");
        harness.run_steps(2);
        let filtered = *height.borrow();
        assert!(
            filtered < SLEEVE_POPUP_HEIGHT,
            "filtered, the picker shrinks to its few sleeves, got {filtered}"
        );

        for _ in 0.."19".len() {
            harness.key_press(egui::Key::Backspace);
        }
        harness.run_steps(2);
        assert!(
            (*height.borrow() - unfiltered).abs() < 1.0,
            "the filter cleared, the picker must be full size again: {} vs {}",
            *height.borrow(),
            unfiltered
        );
    }

    /// The popup of the combo stays bounded: a tall grid must not make it grow to the
    /// screen with the grid stuck at the top.
    #[test]
    fn sleeve_combo_popup_stays_bounded() {
        use std::cell::RefCell;

        let sleeves = fake_sleeves(20);
        let picked: RefCell<Option<String>> = RefCell::new(None);
        let popup = RefCell::new(Vec::new());
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let button = ui.add(egui::Button::new("open"));
                egui::Popup::from_toggle_button_response(&button)
                    .id(egui::Id::new("probe"))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        let mut picked = picked.borrow_mut();
                        sleeve_grid(ui, egui::Id::new("probe_grid"), &sleeves, "", &mut picked);
                        popup.borrow_mut().push(ui.min_rect());
                    });
            });
        harness.run_steps(2);
        harness.get_by_label("open").click();
        harness.run_steps(5);

        let heights: Vec<f32> = popup.borrow().iter().map(|r| r.height()).collect();
        assert!(!heights.is_empty(), "the popup never opened");
        for (i, h) in heights.iter().enumerate() {
            assert!(
                *h < 700.0,
                "the popup must stay bounded, frame {i} is {h} tall (window is 900)"
            );
        }
        let first = heights[heights.len() / 2];
        let last = *heights.last().unwrap();
        assert!(
            (first - last).abs() < 2.0,
            "the popup must not resize while it is open: {first} vs {last}"
        );
    }
    /// From the combo's popup, an image path is copied into the sleeves folder and the new
    /// sleeve picked, even when the folder has no sleeve yet.
    #[test]
    fn sleeve_combo_adds_a_sleeve_from_a_path() {
        use std::cell::RefCell;

        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("My Rem.png");
        image::RgbImage::new(2, 3).save(&image).unwrap();
        let sleeves_dir = dir.path().join("Sleeves");

        let picked: RefCell<Option<String>> = RefCell::new(None);
        // Drawn once the app's fonts are installed: the labels use them.
        let ready = std::cell::Cell::new(false);
        let adder = RefCell::new(SleeveAdder {
            dir: sleeves_dir.clone(),
            ..SleeveAdder::default()
        });
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600.0, 900.0))
            .build_ui(|ui| {
                let mut picked = picked.borrow_mut();
                if ready.get() && picked.is_none() {
                    sleeve_combo(
                        ui,
                        egui::Id::new("add_test"),
                        200.0,
                        &[],
                        "",
                        &mut picked,
                        &mut adder.borrow_mut(),
                    );
                }
            });
        crate::app::theme::install(&harness.ctx);
        harness.run_steps(1);
        ready.set(true);
        harness.run_steps(2);
        harness.get_by_label("No sleeves found \u{25be}").click();
        harness.run_steps(2);
        // The fields of the popup: the sleeve filter, then the image's path and its name.
        let fields: Vec<_> = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .collect();
        assert_eq!(fields.len(), 3);
        fields[1].click();
        harness.run_steps(1);
        let fields: Vec<_> = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .collect();
        fields[1].type_text(&image.display().to_string());
        harness.run_steps(2);
        harness
            .get_all_by_label_contains("Add")
            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
            .expect("no Add button")
            .click();
        for _ in 0..50 {
            harness.run_steps(1);
            if picked.borrow().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert_eq!(picked.borrow().as_deref(), Some("My Rem.png"));
        assert!(sleeves_dir.join("My Rem.png").is_file());
        assert!(adder.borrow().error.is_none());
    }
}
