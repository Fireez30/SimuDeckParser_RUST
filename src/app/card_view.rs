//! Card details (replaces the rich-text HTML of the Qt version) and the preview side pane.

use std::sync::Arc;

use egui::{CornerRadius, Image, Margin, RichText, ScrollArea, Sense, Ui, vec2};
use egui_material_icons::icons::{
    ICON_ALARM, ICON_CONTENT_COPY, ICON_HUB, ICON_KEEP, ICON_KEEP_OFF, ICON_STYLE, ICON_TOUCH_APP,
};
use simu_deck_parser::codes::NameIndex;
use simu_deck_parser::model::{Card, CardType};
use simu_deck_parser::text::{
    KEYWORDS, Rich, parse_rich, split_names, sprite_cost, sprite_trigger, sprite_word,
};

use super::theme::{self, scheme};
use crate::assets;

/// Height / width of a card picture.
pub const CARD_RATIO: f32 = 313.0 / 224.0;

enum Token {
    Icon(&'static egui::ImageSource<'static>, egui::Vec2),
    Bold(&'static str),
}

/// Keyword -> icon, as in the Qt version's HTML rewriting; other keywords (`KEYWORDS`) become
/// bold words.
const ICONS: &[(&str, egui::ImageSource<'static>, egui::Vec2)] = &[
    ("Auto:", assets::AUTO, vec2(28.0, 14.0)),
    ("Cont:", assets::CONT, vec2(28.0, 14.0)),
    ("Act:", assets::ACT, vec2(28.0, 14.0)),
    ("CxCombo", assets::CX_COMBO, vec2(52.0, 14.0)),
];

/// Style of a run of card text, from the simulator's rich-text tags.
#[derive(Clone, Copy)]
struct Style {
    bold: bool,
    italic: bool,
    highlight: bool,
    strike: bool,
}

impl Style {
    fn text(self, ui: &Ui, text: &str) -> RichText {
        let mut t = RichText::new(text);
        if self.bold {
            t = t.font(theme::medium(14.0));
        }
        if self.italic {
            t = t.italics();
        }
        if self.highlight {
            t = t.color(scheme(ui).error);
        }
        if self.strike {
            t = t.strikethrough();
        }
        t
    }
}

fn names_id() -> egui::Id {
    egui::Id::new("card_names")
}

/// Makes the loaded cards' names known to effect text, so quoted names show their card on hover.
pub fn set_names(ctx: &egui::Context, names: Option<Arc<NameIndex>>) {
    ctx.data_mut(|d| match names {
        Some(names) => {
            d.insert_temp(names_id(), names);
        }
        None => {
            d.remove::<Arc<NameIndex>>(names_id());
            d.remove::<Arc<RelatedView>>(related_id());
        }
    });
}

fn names(ctx: &egui::Context) -> Option<Arc<NameIndex>> {
    ctx.data(|d| d.get_temp::<Arc<NameIndex>>(names_id()))
}

/// Renders one line of card text: rich-text tags, then keywords turned into icons or bold words.
/// Quoted names of other cards are highlighted and preview that card on hover (`names`).
fn text_line(ui: &mut Ui, line: &str, card: &Card, names: Option<&NameIndex>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for run in parse_rich(line) {
            match run {
                Rich::Sprite(name) => sprite(ui, name),
                Rich::Text {
                    text,
                    bold,
                    italic,
                    highlight,
                    strike,
                } => {
                    let style = Style {
                        bold,
                        italic,
                        highlight,
                        strike,
                    };
                    let Some(names) = names else {
                        keywords(ui, text, style);
                        continue;
                    };
                    for (span, name) in split_names(text, |n| names.get(n, card).is_some()) {
                        match name.and_then(|n| names.get(n, card)) {
                            Some(target) => card_link(ui, span, target, style),
                            None => keywords(ui, span, style),
                        }
                    }
                }
            }
        }
    });
}

fn keywords(ui: &mut Ui, mut text: &str, style: Style) {
    while !text.is_empty() {
        let icons = ICONS
            .iter()
            .map(|(pat, src, size)| (*pat, Token::Icon(src, *size)));
        let words = KEYWORDS.iter().map(|&(pat, word)| (pat, Token::Bold(word)));
        let next = icons
            .chain(words)
            .filter_map(|(pat, token)| text.find(pat).map(|i| (i, pat, token)))
            .min_by_key(|(i, pat, _)| (*i, std::cmp::Reverse(pat.len())));
        let Some((i, pat, token)) = next else {
            ui.label(style.text(ui, text));
            break;
        };
        if i > 0 {
            ui.label(style.text(ui, &text[..i]));
        }
        match token {
            Token::Icon(src, size) => {
                ui.add(
                    Image::new(src.clone())
                        .fit_to_exact_size(size)
                        .corner_radius(CornerRadius::same(3)),
                );
                ui.label(" ");
            }
            Token::Bold(word) => {
                let bold = Style {
                    bold: true,
                    ..style
                };
                ui.label(bold.text(ui, word));
            }
        }
        text = &text[i + pat.len()..];
    }
}

/// The open card-link panel: the link it belongs to, and when the pointer was last over the
/// link or the panel.
#[derive(Clone)]
struct LinkPanel {
    link: egui::Id,
    last_inside: f64,
    /// Size of the panel last frame, to keep it on screen.
    size: egui::Vec2,
}

fn link_panel_id() -> egui::Id {
    egui::Id::new("card_link_panel")
}

/// Time the pointer may spend off both the link and its panel (crossing the gap between them)
/// before the panel closes.
const LINK_PANEL_GRACE: f64 = 0.3;

/// A quoted card name: highlighted; hovering it opens a panel with the card, which stays open
/// while the pointer is over the name or the panel.
fn card_link(ui: &mut Ui, span: &str, target: &Card, style: Style) {
    let color = scheme(ui).primary;
    let link = ui
        .label(style.text(ui, span).color(color).underline())
        .on_hover_cursor(egui::CursorIcon::Help);
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let mut panel = ctx.data(|d| d.get_temp::<LinkPanel>(link_panel_id()));
    if link.hovered() {
        let size = panel
            .as_ref()
            .filter(|p| p.link == link.id)
            .map_or(vec2(340.0, 600.0), |p| p.size);
        panel = Some(LinkPanel {
            link: link.id,
            last_inside: now,
            size,
        });
    }
    let Some(mut panel) = panel.filter(|p| p.link == link.id) else {
        return;
    };
    if now - panel.last_inside > LINK_PANEL_GRACE {
        ctx.data_mut(|d| d.remove::<LinkPanel>(link_panel_id()));
        return;
    }

    // Beside the name, on the side with room, as low as the whole panel fits.
    let screen = ctx.content_rect().shrink(8.0);
    let top = link
        .rect
        .top()
        .min(screen.bottom() - panel.size.y)
        .max(screen.top());
    let (pivot, x) = if screen.right() - link.rect.right() > panel.size.x + 4.0 {
        (egui::Align2::LEFT_TOP, link.rect.right() + 4.0)
    } else {
        (egui::Align2::RIGHT_TOP, link.rect.left() - 4.0)
    };
    let pos = egui::pos2(x, top);
    let s = scheme(ui);
    let mut related = false;
    let mut content_height = 0.0;
    let area = egui::Area::new(link_panel_id())
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(pos)
        .constrain(true)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(s.surface_container_high)
                .stroke(egui::Stroke::new(1.0, s.outline_variant))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(12))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| {
                    // Scrolls on small screens; `top` makes room for all of it when possible.
                    let max = screen.height() - 26.0;
                    let scroll = ScrollArea::vertical()
                        .max_height(max)
                        .min_scrolled_height((panel.size.y - 26.0).min(max))
                        .show(ui, |ui| {
                            ui.set_width(300.0);
                            related = related_button(ui, target);
                            ui.add_space(8.0);
                            card_preview(ui, target);
                        });
                    content_height = scroll.content_size.y;
                });
        });
    if related {
        ctx.data_mut(|d| d.remove::<LinkPanel>(link_panel_id()));
        return;
    }
    let pointer = ctx.pointer_latest_pos();
    if link.hovered() || pointer.is_some_and(|p| area.response.rect.contains(p)) {
        panel.last_inside = now;
    }
    // Frame margins and stroke around the content.
    panel.size = vec2(area.response.rect.width(), content_height + 26.0);
    ctx.data_mut(|d| d.insert_temp(link_panel_id(), panel));
    // Closes on time even when the pointer stops moving.
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

/// The cards related to one card, shown in their own window.
struct RelatedView {
    target: Card,
    named: Vec<Card>,
    naming: Vec<Card>,
}

fn related_id() -> egui::Id {
    egui::Id::new("related_cards")
}

/// Card clicked in the related cards window, for the detail pane on screen to pin.
fn pin_request_id() -> egui::Id {
    egui::Id::new("related_cards_pin")
}

/// A card clicked in the related cards window is waiting for a detail pane to show it.
pub fn pin_requested(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Card>(pin_request_id()).is_some())
}

/// Code of the card pinned in the detail pane on screen, highlighted in the window.
fn pinned_id() -> egui::Id {
    egui::Id::new("detail_pane_pinned")
}

/// Shows the cards related to `target` in the related cards window.
fn open_related(ctx: &egui::Context, target: &Card) {
    let Some(names) = names(ctx) else {
        return;
    };
    let related = names.related(target);
    let owned = |cards: Vec<&Card>| cards.into_iter().cloned().collect();
    let view = RelatedView {
        target: target.clone(),
        named: owned(related.named),
        naming: owned(related.naming),
    };
    ctx.data_mut(|d| d.insert_temp(related_id(), Arc::new(view)));
}

/// "Related cards" button: opens the window listing the cards that react with `card`.
/// Returns whether it was clicked.
fn related_button(ui: &mut Ui, card: &Card) -> bool {
    let clicked = theme::tonal(Some(ICON_HUB), "Related cards")
        .show(ui)
        .on_hover_text("Cards that name this one, and the ones it names")
        .clicked();
    if clicked {
        open_related(ui.ctx(), card);
    }
    clicked
}

/// Window listing the cards that react with the card whose "Related cards" was clicked: the
/// ones it names and the ones that name it. Clicking one shows it in the detail pane.
pub fn related_window(ctx: &egui::Context) {
    let Some(view) = ctx.data(|d| d.get_temp::<Arc<RelatedView>>(related_id())) else {
        return;
    };
    let target = &view.target;
    let pinned = ctx.data(|d| d.get_temp::<String>(pinned_id()));
    let mut open = true;
    let mut clicked = None;
    egui::Window::new("Related cards")
        .id(related_id())
        .open(&mut open)
        .collapsible(false)
        .default_size([760.0, 600.0])
        .frame(theme::dialog_frame(ctx).inner_margin(Margin::same(20)))
        .show(ctx, |ui| {
            let s = scheme(ui);
            ui.horizontal(|ui| {
                ui.add(
                    assets::card_image(target)
                        .fit_to_exact_size(vec2(40.0, 40.0 * CARD_RATIO))
                        .corner_radius(CornerRadius::same(4)),
                )
                .on_hover_ui(|ui| card_preview(ui, target));
                ui.vertical(|ui| {
                    ui.label(theme::title_medium(&target.name).color(s.on_surface));
                    ui.label(theme::label(&target.key).color(s.on_surface_variant));
                });
            });
            ui.label(
                theme::body_small("Click a card to see its details in the side panel.")
                    .color(s.on_surface_variant),
            );
            ui.add_space(8.0);
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for (title, cards, empty) in [
                    (
                        "Named in its text",
                        &view.named,
                        "Its text names no other card.",
                    ),
                    ("Naming it", &view.naming, "No card of its series names it."),
                ] {
                    theme::section_header(ui, title, Some(cards.len()));
                    if cards.is_empty() {
                        ui.label(theme::body_small(empty).color(s.on_surface_variant));
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                        for card in cards {
                            let selected = pinned.as_deref() == Some(card.key.as_str());
                            if related_thumbnail(ui, card, selected).clicked() {
                                clicked = Some(card.clone());
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        });
    if !open {
        ctx.data_mut(|d| d.remove::<Arc<RelatedView>>(related_id()));
    } else if let Some(card) = clicked {
        ctx.data_mut(|d| d.insert_temp(pin_request_id(), card));
    }
}

/// Picture and name of a related card; hover previews it. `selected`: shown in the detail pane.
fn related_thumbnail(ui: &mut Ui, card: &Card, selected: bool) -> egui::Response {
    let width = 104.0;
    let s = scheme(ui);
    let inner = ui.allocate_ui(vec2(width, width * CARD_RATIO + 40.0), |ui| {
        ui.vertical(|ui| {
            ui.set_width(width);
            ui.add(
                assets::card_image(card)
                    .fit_to_exact_size(vec2(width, width * CARD_RATIO))
                    .corner_radius(CornerRadius::same(6)),
            );
            ui.add(egui::Label::new(theme::body_small(&card.name).color(s.on_surface)).truncate());
            ui.add(
                egui::Label::new(theme::body_small(&card.key).color(s.on_surface_variant))
                    .truncate(),
            );
        })
    });
    let r = ui
        .interact(inner.response.rect, ui.id().with(&card.key), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_ui(|ui| card_preview(ui, card));
    if selected || r.hovered() {
        let (width, color) = if selected {
            (3.0, s.primary)
        } else {
            (2.0, s.outline)
        };
        ui.painter().rect_stroke(
            inner.response.rect.expand(3.0),
            CornerRadius::same(8),
            egui::Stroke::new(width, color),
            egui::StrokeKind::Outside,
        );
    }
    r
}

/// `<sprite name="…"/>`: stock costs, card states, trigger icons, keywords.
fn sprite(ui: &mut Ui, name: &str) {
    let s = scheme(ui);
    if let Some(cost) = sprite_cost(name) {
        let (rect, response) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 8.5, s.primary);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            cost.to_string(),
            theme::medium(11.0),
            s.on_primary,
        );
        response.on_hover_text(format!("Pay {cost} stock"));
        return;
    }
    let trigger = sprite_trigger(name);
    if let Some(t) = trigger {
        ui.add(Image::new(assets::trigger_icon(t)).fit_to_exact_size(vec2(14.0, 17.0)))
            .on_hover_text(t.label());
        return;
    }
    let word = match name {
        // Always followed by the ALARM keyword itself.
        "alarm" => {
            ui.label(ICON_ALARM.rich_text().size(16.0).color(s.primary))
                .on_hover_text("Alarm");
            ui.label(" ");
            return;
        }
        _ => sprite_word(name),
    };
    theme::wrap_before(ui, &word, 8.0);
    egui::Frame::new()
        .fill(s.secondary_container)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.label(theme::label(word).color(s.on_secondary_container));
        });
}

/// Name, code, stats as chips and traits.
pub fn card_summary(ui: &mut Ui, card: &Card) {
    summary(ui, card, true);
}

/// Hover preview (deck builder): picture, summary and text, with nothing clickable, so the
/// tooltip never catches clicks meant for the cards around it.
pub fn card_preview(ui: &mut Ui, card: &Card) {
    ui.set_max_width(300.0);
    ui.add(
        assets::card_image(card)
            .fit_to_exact_size(vec2(260.0, 260.0 * CARD_RATIO))
            .corner_radius(CornerRadius::same(10)),
    );
    ui.add_space(6.0);
    summary(ui, card, false);
    effect_text(ui, card, false);
}

fn summary(ui: &mut Ui, card: &Card, copy_button: bool) {
    let s = scheme(ui);
    let dark = ui.visuals().dark_mode;
    ui.add(egui::Label::new(theme::title(&card.name).color(s.on_surface)).wrap());
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&card.key)
                .font(theme::medium(13.0))
                .color(s.on_surface_variant),
        );
        if copy_button
            && theme::small_icon_button(ui, ICON_CONTENT_COPY, "Copy card code").clicked()
        {
            ui.ctx().copy_text(card.key.clone());
        }
    });
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        theme::info_chip(
            ui,
            None,
            card.color.label(),
            Some(theme::card_color(card.color, dark)),
        );
        theme::info_chip(ui, None, card.card_type.label(), None);
        if card.card_type != CardType::Climax {
            theme::info_chip(ui, None, &format!("Level {}", card.level), None);
            theme::info_chip(ui, None, &format!("Cost {}", card.cost), None);
        }
        if card.card_type == CardType::Character {
            theme::info_chip(ui, None, &format!("Power {}", card.power), None);
        }
    });

    if !card.triggers.is_empty() || card.soul_count > 0 {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if !card.triggers.is_empty() {
                ui.label(theme::label("Triggers").color(s.on_surface_variant));
                for t in &card.triggers {
                    ui.add(
                        Image::new(assets::trigger_icon(*t)).fit_to_exact_size(vec2(16.0, 20.0)),
                    )
                    .on_hover_text(t.label());
                }
                ui.add_space(12.0);
            }
            if card.soul_count > 0 {
                ui.label(theme::label("Soul").color(s.on_surface_variant));
                for _ in 0..card.soul_count {
                    ui.add(Image::new(assets::SOUL).fit_to_exact_size(vec2(16.0, 20.0)));
                }
            }
        });
    }

    if !card.traits.is_empty() {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for t in &card.traits {
                egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0, s.outline_variant))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.label(theme::label(t).color(s.on_surface_variant));
                    });
            }
        });
    }
}

/// Effect text, with keyword icons and hoverable card names.
pub fn card_text(ui: &mut Ui, card: &Card) {
    effect_text(ui, card, true);
}

/// `links`: quoted card names preview their card on hover (not inside tooltips, which can't be
/// hovered).
fn effect_text(ui: &mut Ui, card: &Card, links: bool) {
    let s = scheme(ui);
    let names = links.then(|| names(ui.ctx())).flatten();
    if !card.text.trim().is_empty() {
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(s.surface_container)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for line in card.text.lines().filter(|l| !l.trim().is_empty()) {
                    text_line(ui, line, card, names.as_deref());
                }
            });
    }
}

/// Side pane showing the hovered card, or the pinned one (click pins, Esc or the pin button unpins).
#[derive(Default)]
pub struct DetailPane {
    card: Option<Card>,
    pinned: bool,
}

impl DetailPane {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn pinned_key(&self) -> Option<&str> {
        self.card
            .as_ref()
            .filter(|_| self.pinned)
            .map(|c| c.key.as_str())
    }

    pub fn hover(&mut self, card: &Card) {
        if !self.pinned && self.card.as_ref().is_none_or(|c| c.key != card.key) {
            self.card = Some(card.clone());
        }
    }

    /// Clicking the pinned card again unpins it.
    pub fn click(&mut self, card: &Card) {
        if self.pinned_key() == Some(&card.key) {
            self.pinned = false;
        } else {
            self.card = Some(card.clone());
            self.pinned = true;
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pinned = false;
        }
        // A card clicked in the related cards window.
        let request = ui.ctx().data_mut(|d| {
            let card = d.get_temp::<Card>(pin_request_id());
            d.remove::<Card>(pin_request_id());
            card
        });
        if let Some(card) = request {
            self.card = Some(card);
            self.pinned = true;
        }
        let pinned = self.pinned_key().map(str::to_owned);
        ui.ctx().data_mut(|d| match pinned {
            Some(key) => {
                d.insert_temp(pinned_id(), key);
            }
            None => d.remove::<String>(pinned_id()),
        });
        let s = scheme(ui);
        let Some(card) = &self.card else {
            theme::empty_state(
                ui,
                ICON_TOUCH_APP,
                "No card selected",
                "Hover a card to preview it.\nClick to pin it here.",
            );
            return;
        };
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (text, icon) = if self.pinned {
                    ("Pinned", ICON_KEEP)
                } else {
                    ("Preview", ICON_STYLE)
                };
                ui.label(icon.rich_text().size(18.0).color(s.on_surface_variant));
                ui.label(theme::label(text).color(s.on_surface_variant));
                if self.pinned {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::small_icon_button(ui, ICON_KEEP_OFF, "Unpin (Esc)").clicked() {
                            self.pinned = false;
                        }
                    });
                }
            });
            ui.add_space(4.0);
            let width = ui.available_width().min(340.0);
            ui.vertical_centered(|ui| {
                ui.add(
                    assets::card_image(card)
                        .fit_to_exact_size(vec2(width, width * CARD_RATIO))
                        .corner_radius(CornerRadius::same(12)),
                );
            });
            ui.add_space(12.0);
            card_summary(ui, card);
            ui.add_space(8.0);
            related_button(ui, card);
            card_text(ui, card);
        });
    }
}
