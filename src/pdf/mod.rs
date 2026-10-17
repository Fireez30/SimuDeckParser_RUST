//! Printable PDFs of a deck: proxies to cut out and slip in sleeves, and a translation sheet
//! (each card with its English text).
//!
//! This file lays out the two printouts; the submodules do the drawing:
//! `document.rs` (pages, lines, wrapped text), `font.rs` (embedded fonts) and `artwork.rs`
//! (card pictures).

mod artwork;
mod document;
mod font;

use anyhow::Result;

use crate::filters::sort_for_deck;
use crate::model::{CARD_IMAGE_RATIO, Card, CardType, Deck};
use crate::text::{KEYWORDS, Rich, parse_rich, sprite_cost, sprite_trigger, sprite_word};
use artwork::{Fit, prepare_art};
use document::{Block, Doc, Rgb, Run, block};

/// Artwork resolution in the PDFs.
const PROXY_DPI: f32 = 300.0;
const SHEET_DPI: f32 = 200.0;
const BLACK: Rgb = [0.1, 0.1, 0.1];
const GREY: Rgb = [0.38, 0.38, 0.4];
const RED: Rgb = [0.78, 0.1, 0.1];

/// Millimetres to points, the PDF unit (1/72 inch).
fn mm(v: f32) -> f32 {
    v * 72.0 / 25.4
}

/// Weiss Schwarz card size, in millimetres.
const CARD_MM: (f32, f32) = (59.0, 86.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    pub const ALL: [Paper; 2] = [Paper::A4, Paper::Letter];

    pub fn label(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::Letter => "US Letter",
        }
    }

    /// Width and height in points.
    fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (mm(210.0), mm(297.0)),
            Paper::Letter => (612.0, 792.0),
        }
    }
}

/// Proxies per row and per column on a page, leaving room for the crop marks.
pub fn proxy_grid(paper: Paper) -> (usize, usize) {
    let (w_mm, h_mm) = CARD_MM;
    let (width, height) = paper.size();
    let min_margin = mm(5.0);
    let fit =
        |page: f32, card: f32| (((page - 2.0 * min_margin) / mm(card)).floor() as usize).max(1);
    (fit(width, w_mm), fit(height, h_mm))
}

/// Every card of the deck at Weiss Schwarz size, as many per page as fit, with crop marks
/// between them. Cards without artwork are printed as text.
pub fn proxies(deck: &Deck, paper: Paper) -> Result<Vec<u8>> {
    let mut cards = deck.cards.clone();
    sort_for_deck(&mut cards);
    let (w_mm, h_mm) = CARD_MM;
    let art = prepare_art(&unique(&cards), w_mm, h_mm, PROXY_DPI, Fit::Fill);

    let mut doc = Doc::new(paper);
    let (cw, ch) = (mm(w_mm), mm(h_mm));
    let (cols, rows) = proxy_grid(paper);
    let x0 = (doc.width - cols as f32 * cw) / 2.0;
    let y0 = (doc.height - rows as f32 * ch) / 2.0;

    for page in cards.chunks(cols * rows) {
        doc.new_page();
        for (i, card) in page.iter().enumerate() {
            let x = x0 + (i % cols) as f32 * cw;
            let y = y0 + (i / cols) as f32 * ch;
            match art.get(&card.key) {
                Some(image) => doc.image(&card.key, image, x, y, cw, ch),
                None => text_card(&mut doc, card, x, y, cw, ch),
            }
        }
        let used_rows = page.len().div_ceil(cols);
        let used_cols = page.len().min(cols);
        crop_marks(&mut doc, x0, y0, cw, ch, used_cols, used_rows);
    }
    doc.finish(&format!("{} (proxies)", deck.name))
}

/// Marks outside the grid on every cut line: a paper cutter lines up on them.
fn crop_marks(doc: &mut Doc, x0: f32, y0: f32, cw: f32, ch: f32, cols: usize, rows: usize) {
    let gap = mm(1.0);
    let (x1, y1) = (x0 + cols as f32 * cw, y0 + rows as f32 * ch);
    let len_x = (x0 - gap - mm(1.0)).min(mm(6.0));
    let len_y = (y0 - gap - mm(1.0)).min(mm(6.0));
    for i in 0..=cols {
        let x = x0 + i as f32 * cw;
        doc.line(x, y0 - gap, x, y0 - gap - len_y, 0.3, BLACK);
        doc.line(x, y1 + gap, x, y1 + gap + len_y, 0.3, BLACK);
    }
    for j in 0..=rows {
        let y = y0 + j as f32 * ch;
        doc.line(x0 - gap, y, x0 - gap - len_x, y, 0.3, BLACK);
        doc.line(x1 + gap, y, x1 + gap + len_x, y, 0.3, BLACK);
    }
}

/// A proxy without artwork: name, stats and effect text in a frame.
fn text_card(doc: &mut Doc, card: &Card, x: f32, y: f32, w: f32, h: f32) {
    doc.rect(x, y, w, h, 0.5, GREY);
    let pad = mm(3.0);
    let width = w - 2.0 * pad;
    let mut top = y + pad;
    for block in card_blocks(card, 9.0, 6.5) {
        top = doc.paragraph(&block, x + pad, top, width, y + h - pad);
    }
}

/// Each different card once, with its copies, picture, stats and effect text.
pub fn translation(deck: &Deck, paper: Paper) -> Result<Vec<u8>> {
    let mut cards = deck.cards.clone();
    sort_for_deck(&mut cards);
    let mut counts: Vec<(&Card, usize)> = Vec::new();
    for card in &cards {
        match counts.iter_mut().find(|(c, _)| c.key == card.key) {
            Some((_, n)) => *n += 1,
            None => counts.push((card, 1)),
        }
    }
    let img_w = 26.0;
    let art = prepare_art(
        &unique(&cards),
        img_w,
        img_w * CARD_IMAGE_RATIO,
        SHEET_DPI,
        Fit::Contain,
    );

    let mut doc = Doc::new(paper);
    let margin = mm(14.0);
    let bottom = doc.height - margin - mm(4.0);
    let (img_w, img_h) = (mm(img_w), mm(img_w * CARD_IMAGE_RATIO));
    let text_x = margin + img_w + mm(5.0);
    let text_w = doc.width - margin - text_x;

    doc.new_page();
    let mut y = margin;
    let title = vec![Run::new(&deck.name, true, BLACK)];
    y = doc.paragraph(
        &block(title, 16.0),
        margin,
        y,
        doc.width - 2.0 * margin,
        bottom,
    );
    let subtitle = format!(
        "{} cards, {} different. Translation sheet made with SimuDeckParser.",
        cards.len(),
        counts.len()
    );
    let subtitle = vec![Run::new(&subtitle, false, GREY)];
    y = doc.paragraph(&block(subtitle, 9.0), margin, y + mm(1.0), text_w, bottom) + mm(4.0);

    let mut kind = None;
    for (card, copies) in counts {
        let mut blocks = vec![block(
            vec![
                Run::new(&format!("{copies}× "), true, RED),
                Run::new(&card.name, true, BLACK),
            ],
            10.5,
        )];
        blocks.extend(card_blocks(card, 0.0, 8.5));
        let text_h = blocks
            .iter()
            .map(|b| doc.paragraph_height(b, text_w))
            .sum::<f32>();
        let row_h = img_h.max(text_h);
        let header = (kind != Some(card.card_type)).then(|| {
            let total: usize = cards
                .iter()
                .filter(|c| c.card_type == card.card_type)
                .count();
            format!("{} ({total})", card.card_type.plural_label())
        });
        let header_h = if header.is_some() { mm(10.0) } else { 0.0 };
        if y + header_h + row_h > bottom && y > margin + mm(20.0) {
            doc.new_page();
            y = margin;
        }
        if let Some(header) = header {
            kind = Some(card.card_type);
            let runs = vec![Run::new(&header, true, BLACK)];
            y = doc.paragraph(&block(runs, 12.0), margin, y + mm(2.0), text_w, bottom);
            doc.line(
                margin,
                y + mm(1.0),
                doc.width - margin,
                y + mm(1.0),
                0.6,
                GREY,
            );
            y += mm(4.0);
        }
        match art.get(&card.key) {
            Some(image) => {
                // Whole picture, centred at the top of its box (climaxes are landscape).
                let scale = (img_w / image.width as f32).min(img_h / image.height as f32);
                let (w, h) = (image.width as f32 * scale, image.height as f32 * scale);
                doc.image(&card.key, image, margin + (img_w - w) / 2.0, y, w, h);
            }
            None => doc.rect(margin, y, img_w, img_h, 0.5, GREY),
        }
        let mut top = y;
        for b in &blocks {
            top = doc.paragraph(b, text_x, top, text_w, f32::MAX);
        }
        y += row_h + mm(3.0);
        doc.line(
            text_x,
            y - mm(1.5),
            doc.width - margin,
            y - mm(1.5),
            0.25,
            [0.8; 3],
        );
    }
    doc.finish(&format!("{} (translation)", deck.name))
}

/// Text of a card: name (skipped when `name_size` is 0), stats line, traits, effects.
fn card_blocks(card: &Card, name_size: f32, size: f32) -> Vec<Block> {
    let mut blocks = Vec::new();
    if name_size > 0.0 {
        blocks.push(block(vec![Run::new(&card.name, true, BLACK)], name_size));
    }
    let mut stats = vec![card.key.clone(), card.color.label().to_string()];
    stats.push(card.card_type.label().to_string());
    if card.card_type != CardType::Climax {
        stats.push(format!("Level {}", card.level));
        stats.push(format!("Cost {}", card.cost));
    }
    if card.card_type == CardType::Character {
        stats.push(format!("Power {}", card.power));
    }
    if card.soul_count > 0 {
        stats.push(format!("Soul {}", card.soul_count));
    }
    if !card.triggers.is_empty() {
        let triggers: Vec<&str> = card.triggers.iter().map(|t| t.label()).collect();
        stats.push(format!("Trigger {}", triggers.join(" + ")));
    }
    let small = size - 1.0;
    blocks.push(block(
        vec![Run::new(&stats.join(" · "), false, GREY)],
        small,
    ));
    if !card.traits.is_empty() {
        let traits = format!("Traits: {}", card.traits.join(", "));
        blocks.push(block(vec![Run::new(&traits, false, GREY)], small));
    }
    for line in card.text.lines().filter(|l| !l.trim().is_empty()) {
        let mut b = block(effect_runs(line), size);
        b.space_before = size * 0.35;
        blocks.push(b);
    }
    blocks
}

/// A line of effect text: rich-text tags, sprites and keywords as plain or bold text.
fn effect_runs(line: &str) -> Vec<Run> {
    const ICONS: [(&str, &str); 4] = [
        ("Auto:", "AUTO"),
        ("Cont:", "CONT"),
        ("Act:", "ACT"),
        ("CxCombo", "CX COMBO"),
    ];
    let mut runs = Vec::new();
    for rich in parse_rich(line) {
        match rich {
            Rich::Sprite(name) => {
                let word = if let Some(cost) = sprite_cost(name) {
                    format!("({cost})")
                } else if let Some(t) = sprite_trigger(name) {
                    format!("[{}]", t.label())
                } else if name == "alarm" {
                    // Always followed by the ALARM keyword itself.
                    continue;
                } else {
                    format!("[{}]", sprite_word(name))
                };
                runs.push(Run::new(&word, true, BLACK));
            }
            Rich::Text(mut text, style) => {
                let bold = style.bold;
                let color = if style.highlight { RED } else { BLACK };
                while !text.is_empty() {
                    let next = ICONS
                        .iter()
                        .chain(KEYWORDS)
                        .filter_map(|&(pat, word)| text.find(pat).map(|i| (i, pat, word)))
                        .min_by_key(|(i, pat, _)| (*i, std::cmp::Reverse(pat.len())));
                    let Some((i, pat, word)) = next else {
                        runs.push(Run::new(text, bold, color));
                        break;
                    };
                    if i > 0 {
                        runs.push(Run::new(&text[..i], bold, color));
                    }
                    runs.push(Run::new(word, true, color));
                    text = &text[i + pat.len()..];
                }
            }
        }
    }
    runs
}

/// Each different card once, in order.
fn unique(cards: &[Card]) -> Vec<&Card> {
    let mut out: Vec<&Card> = Vec::new();
    for card in cards {
        if !out.iter().any(|c| c.key == card.key) {
            out.push(card);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Trigger};

    fn card(key: &str, card_type: CardType, image: Option<std::path::PathBuf>) -> Card {
        Card {
            key: key.into(),
            card_type,
            name: format!("\"Card\" {key}"),
            image,
            color: Color::Blue,
            level: 1,
            cost: 1,
            power: 5000,
            triggers: vec![Trigger::Soul],
            soul_count: 1,
            traits: vec!["Magic".into()],
            code: String::new(),
            text: "Auto: [<sprite name=\"1cost\"/>] When this card becomes \
                   <sprite name=\"reverse\"/>, <b>ENCORE</b> <color=red>Once per turn.</color>\n\
                   Cont: ASSIST"
                .into(),
        }
    }

    fn pages(pdf: &[u8]) -> usize {
        String::from_utf8_lossy(pdf)
            .matches("/Type /Page\n")
            .count()
    }

    #[test]
    fn printouts() {
        let dir = tempfile::tempdir().unwrap();
        let portrait = dir.path().join("a.png");
        let landscape = dir.path().join("b.jpg");
        image::RgbaImage::from_pixel(224, 313, image::Rgba([200, 30, 30, 128]))
            .save(&portrait)
            .unwrap();
        image::RgbImage::from_pixel(313, 224, image::Rgb([30, 30, 200]))
            .save(&landscape)
            .unwrap();
        let cards = (0..20)
            .map(|i| {
                let (t, image) = match i % 7 {
                    0 => (CardType::Climax, Some(landscape.clone())),
                    1 => (CardType::Character, None),
                    _ => (CardType::Character, Some(portrait.clone())),
                };
                card(&format!("KS/W49-E{:03}", i % 7), t, image)
            })
            .collect();
        let deck = Deck {
            name: "Test".into(),
            date: String::new(),
            cards,
            unmatched: Vec::new(),
        };

        assert_eq!(proxy_grid(Paper::A4), (3, 3));
        assert_eq!(proxy_grid(Paper::Letter), (3, 3));
        let proxies = proxies(&deck, Paper::A4).unwrap();
        assert!(proxies.starts_with(b"%PDF-"));
        assert_eq!(pages(&proxies), 3);
        // One picture per different card with artwork, however many copies are printed.
        let text = String::from_utf8_lossy(&proxies);
        assert_eq!(text.matches("/Subtype /Image").count(), 6);

        let sheet = translation(&deck, Paper::Letter).unwrap();
        assert!(sheet.starts_with(b"%PDF-"));
        assert!(pages(&sheet) >= 1);
        assert!(String::from_utf8_lossy(&sheet).contains("/FontFile2"));
    }

    #[test]
    fn effect_text() {
        let runs = effect_runs(
            "Auto: [<sprite name=\"1cost\"/>] <sprite name=\"reverse\"/> <color=red>ALARM</color>",
        );
        let text: Vec<(&str, bool)> = runs
            .iter()
            .map(|r| (r.text.as_str(), r.style.bold))
            .collect();
        assert_eq!(
            text,
            [
                ("AUTO", true),
                (" [", false),
                ("(1)", true),
                ("] ", false),
                ("[Reverse]", true),
                (" ", false),
                ("Alarm", true),
            ]
        );
        assert_eq!(runs.last().unwrap().style.color, RED);
    }
}
