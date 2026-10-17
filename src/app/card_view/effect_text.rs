//! Card effect text as the simulator writes it, drawn with its styles and icons:
//! `<b>…</b>` markup, `<sprite …/>` icons, keywords such as `Auto:` or `ENCORE`, and quoted
//! names of other cards, which show that card when hovered.

use egui::{CornerRadius, Image, Margin, RichText, ScrollArea, Ui, vec2};
use egui_material_icons::icons::ICON_ALARM;
use simu_deck_parser::card_index::NameIndex;
use simu_deck_parser::model::Card;
use simu_deck_parser::text::{
    KEYWORDS, Rich, TextStyle, parse_rich, split_names, sprite_cost, sprite_trigger, sprite_word,
};

use super::card_preview;
use super::related::{names, related_button};
use crate::app::assets;
use crate::app::theme::{self, scheme};

/// What a keyword of card text is drawn as.
enum Token {
    Icon(&'static egui::ImageSource<'static>, egui::Vec2),
    Bold(&'static str),
}

/// Keyword -> icon, as the original Qt tool did; other keywords (`KEYWORDS`) become
/// bold words.
const ICONS: &[(&str, egui::ImageSource<'static>, egui::Vec2)] = &[
    ("Auto:", assets::AUTO, vec2(28.0, 14.0)),
    ("Cont:", assets::CONT, vec2(28.0, 14.0)),
    ("Act:", assets::ACT, vec2(28.0, 14.0)),
    ("CxCombo", assets::CX_COMBO, vec2(52.0, 14.0)),
];

/// `text` in `style`.
fn styled(ui: &Ui, style: TextStyle, text: &str) -> RichText {
    let mut t = RichText::new(text);
    if style.bold {
        t = t.font(theme::medium(14.0));
    }
    if style.italic {
        t = t.italics();
    }
    if style.highlight {
        t = t.color(scheme(ui).error);
    }
    if style.strike {
        t = t.strikethrough();
    }
    t
}

/// The card's effect text, in a rounded box. With `links`, quoted card names preview their card
/// on hover (not inside tooltips, which can't be hovered).
pub(super) fn effect_text(ui: &mut Ui, card: &Card, links: bool) {
    let s = scheme(ui);
    let names = if links { names(ui.ctx()) } else { None };
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

/// Renders one line of card text: rich-text tags, then keywords turned into icons or bold words.
/// Quoted names of other cards are highlighted and preview that card on hover (`names`).
fn text_line(ui: &mut Ui, line: &str, card: &Card, names: Option<&NameIndex>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for run in parse_rich(line) {
            match run {
                Rich::Sprite(name) => sprite(ui, name),
                Rich::Text(text, style) => {
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

/// Draws text, turning the keywords it contains into icons (`Auto:`, `CxCombo`…) or bold
/// words (`ENCORE` -> **Encore**).
fn keywords(ui: &mut Ui, mut text: &str, style: TextStyle) {
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
            ui.label(styled(ui, style, text));
            break;
        };
        if i > 0 {
            ui.label(styled(ui, style, &text[..i]));
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
                let bold = TextStyle {
                    bold: true,
                    ..style
                };
                ui.label(styled(ui, bold, word));
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
fn card_link(ui: &mut Ui, span: &str, target: &Card, style: TextStyle) {
    let color = scheme(ui).primary;
    let link = ui
        .label(styled(ui, style, span).color(color).underline())
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
