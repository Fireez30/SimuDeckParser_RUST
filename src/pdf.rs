//! Printable PDFs of a deck: proxies to cut out and slip in sleeves, and a translation sheet
//! (each card with its English text).

use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::path::Path;

use anyhow::{Context as _, Result};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbImage};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use subsetter::GlyphRemapper;

use crate::filters::{DECK_ORDER, sort_cards};
use crate::model::{Card, CardType, Deck};
use crate::text::{KEYWORDS, Rich, parse_rich, sprite_cost, sprite_trigger, sprite_word};

pub const FONT_REGULAR: &[u8] = include_bytes!("../fonts/NotoSans-Regular.ttf");
pub const FONT_MEDIUM: &[u8] = include_bytes!("../fonts/NotoSans-Medium.ttf");

/// Artwork resolution in the PDFs.
const PROXY_DPI: f32 = 300.0;
const SHEET_DPI: f32 = 200.0;
/// Height / width of the simulator's card pictures.
const ART_RATIO: f32 = 313.0 / 224.0;

const BLACK: [f32; 3] = [0.1, 0.1, 0.1];
const GREY: [f32; 3] = [0.38, 0.38, 0.4];
const RED: [f32; 3] = [0.78, 0.1, 0.1];

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
    sort_cards(&mut cards, DECK_ORDER);
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
    sort_cards(&mut cards, DECK_ORDER);
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
        img_w * ART_RATIO,
        SHEET_DPI,
        Fit::Contain,
    );

    let mut doc = Doc::new(paper);
    let margin = mm(14.0);
    let bottom = doc.height - margin - mm(4.0);
    let (img_w, img_h) = (mm(img_w), mm(img_w * ART_RATIO));
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
            format!("{} ({total})", plural(card.card_type))
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

fn plural(t: CardType) -> &'static str {
    match t {
        CardType::Character => "Characters",
        CardType::Event => "Events",
        CardType::Climax => "Climaxes",
    }
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
            Rich::Text {
                mut text,
                bold,
                highlight,
                ..
            } => {
                let color = if highlight { RED } else { BLACK };
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

fn unique(cards: &[Card]) -> Vec<&Card> {
    let mut out: Vec<&Card> = Vec::new();
    for card in cards {
        if !out.iter().any(|c| c.key == card.key) {
            out.push(card);
        }
    }
    out
}

/// How artwork goes in its `w_mm` × `h_mm` box.
#[derive(Clone, Copy)]
enum Fit {
    /// Fills the box, cropping the edges; landscape pictures (climaxes) are turned upright.
    Fill,
    /// Whole picture, as large as fits.
    Contain,
}

/// Artwork of each card, fitted to `w_mm` × `h_mm` and scaled down to `dpi`, by card key.
/// Cards without a readable picture are left out.
fn prepare_art(cards: &[&Card], w_mm: f32, h_mm: f32, dpi: f32, fit: Fit) -> HashMap<String, Jpeg> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = cards
            .iter()
            .filter_map(|card| Some((card.key.clone(), card.image.as_deref()?)))
            .map(|(key, path)| s.spawn(move || (key, Jpeg::card(path, w_mm, h_mm, dpi, fit))))
            .collect();
        jobs.into_iter()
            .filter_map(|job| job.join().ok())
            .filter_map(|(key, jpeg)| Some((key, jpeg.ok()?)))
            .collect()
    })
}

struct Jpeg {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl Jpeg {
    fn card(path: &Path, w_mm: f32, h_mm: f32, dpi: f32, fit: Fit) -> Result<Self> {
        let image = image::open(path).with_context(|| format!("reading {}", path.display()))?;
        let mut rgb = flatten(image);
        let px = |mm: f32| (mm / 25.4 * dpi).round() as u32;
        let (max_w, max_h) = (px(w_mm), px(h_mm));
        match fit {
            Fit::Fill => {
                if rgb.width() > rgb.height() {
                    rgb = imageops::rotate90(&rgb);
                }
                // Crop to the printed shape, keeping the middle.
                let target = w_mm / h_mm;
                let (w, h) = (rgb.width(), rgb.height());
                let (cw, ch) = if w as f32 / h as f32 > target {
                    ((h as f32 * target).round() as u32, h)
                } else {
                    (w, (w as f32 / target).round() as u32)
                };
                rgb = imageops::crop_imm(&rgb, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image();
                if rgb.width() > max_w {
                    rgb = imageops::resize(&rgb, max_w, max_h, FilterType::Lanczos3);
                }
            }
            Fit::Contain => {
                let scale =
                    (max_w as f32 / rgb.width() as f32).min(max_h as f32 / rgb.height() as f32);
                if scale < 1.0 {
                    let w = (rgb.width() as f32 * scale).round().max(1.0) as u32;
                    let h = (rgb.height() as f32 * scale).round().max(1.0) as u32;
                    rgb = imageops::resize(&rgb, w, h, FilterType::Lanczos3);
                }
            }
        }
        let mut data = Vec::new();
        JpegEncoder::new_with_quality(&mut data, 90).encode_image(&rgb)?;
        Ok(Self {
            data,
            width: rgb.width(),
            height: rgb.height(),
        })
    }
}

/// RGB on a white background.
fn flatten(image: DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        image::Rgb([over(r), over(g), over(b)])
    })
}

#[derive(Clone, Copy, PartialEq)]
struct Style {
    bold: bool,
    color: [f32; 3],
}

struct Run {
    text: String,
    style: Style,
}

impl Run {
    fn new(text: &str, bold: bool, color: [f32; 3]) -> Self {
        Self {
            text: text.to_string(),
            style: Style { bold, color },
        }
    }
}

/// A paragraph: runs of text wrapped together at one size.
struct Block {
    runs: Vec<Run>,
    size: f32,
    space_before: f32,
}

fn block(runs: Vec<Run>, size: f32) -> Block {
    Block {
        runs,
        size,
        space_before: 0.0,
    }
}

/// A wrapped line: pieces of text at their x offset.
type Line = Vec<(f32, String, Style)>;

/// Words and runs of whitespace.
fn words(text: &str) -> impl Iterator<Item = (&str, bool)> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let space = first.is_whitespace();
        let end = rest
            .find(|c: char| c.is_whitespace() != space)
            .unwrap_or(rest.len());
        let (word, tail) = rest.split_at(end);
        rest = tail;
        Some((word, space))
    })
}

/// An embedded font (subset to the glyphs used), addressed by glyph id.
struct Font {
    data: &'static [u8],
    face: ttf_parser::Face<'static>,
    /// Resource name and PostScript name (with a subset tag).
    resource: &'static [u8],
    base: &'static [u8],
    remapper: GlyphRemapper,
    /// Subset glyph id -> width (thousandths of an em) and the character it draws.
    glyphs: BTreeMap<u16, (f32, char)>,
}

impl Font {
    fn new(data: &'static [u8], resource: &'static [u8], base: &'static [u8]) -> Self {
        Self {
            data,
            face: ttf_parser::Face::parse(data, 0).expect("embedded font"),
            resource,
            base,
            remapper: GlyphRemapper::new(),
            glyphs: BTreeMap::new(),
        }
    }

    fn glyph(&self, c: char) -> (ttf_parser::GlyphId, f32) {
        let c = if c.is_whitespace() { ' ' } else { c };
        let id = self.face.glyph_index(c).unwrap_or_default();
        let advance = self.face.glyph_hor_advance(id).unwrap_or(0) as f32;
        (id, advance / self.face.units_per_em() as f32)
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.glyph(c).1).sum::<f32>() * size
    }

    /// Glyph ids for a `show` operation (Identity-H: two bytes each).
    fn encode(&mut self, text: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() * 2);
        for c in text.chars().filter(|c| !c.is_control()) {
            let (id, advance) = self.glyph(c);
            let cid = self.remapper.remap(id.0);
            self.glyphs.entry(cid).or_insert((advance * 1000.0, c));
            out.extend(cid.to_be_bytes());
        }
        out
    }

    fn write(&self, pdf: &mut Pdf, refs: [Ref; 5]) -> Result<()> {
        let [type0, cid, descriptor, file, cmap] = refs;
        let info = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };
        pdf.type0_font(type0)
            .base_font(Name(self.base))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid)
            .to_unicode(cmap);
        let mut font = pdf.cid_font(cid);
        font.subtype(CidFontType::Type2)
            .base_font(Name(self.base))
            .system_info(info)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        let mut widths = font.widths();
        for (&id, &(width, _)) in &self.glyphs {
            widths.consecutive(id, [width]);
        }
        widths.finish();
        font.finish();

        let scale = 1000.0 / self.face.units_per_em() as f32;
        let b = self.face.global_bounding_box();
        pdf.font_descriptor(descriptor)
            .name(Name(self.base))
            .flags(FontFlags::NON_SYMBOLIC)
            .bbox(Rect::new(
                b.x_min as f32 * scale,
                b.y_min as f32 * scale,
                b.x_max as f32 * scale,
                b.y_max as f32 * scale,
            ))
            .italic_angle(0.0)
            .ascent(self.face.ascender() as f32 * scale)
            .descent(self.face.descender() as f32 * scale)
            .cap_height(self.face.capital_height().unwrap_or(700) as f32 * scale)
            .stem_v(80.0)
            .font_file2(file);

        let subset = subsetter::subset(self.data, 0, &self.remapper)
            .map_err(|e| anyhow::anyhow!("subsetting the font: {e:?}"))?;
        pdf.stream(file, &deflate(&subset))
            .filter(Filter::FlateDecode)
            .pair(Name(b"Length1"), subset.len() as i32);

        let mut map = UnicodeCmap::new(Name(b"Custom"), info);
        for (&id, &(_, c)) in &self.glyphs {
            map.pair(id, c);
        }
        pdf.stream(cmap, &map.finish());
        Ok(())
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}

/// A PDF being drawn, in points from the top-left corner of the page.
struct Doc {
    pdf: Pdf,
    last_ref: i32,
    width: f32,
    height: f32,
    fonts: [Font; 2],
    /// Image XObjects, named `Im{index}`, by card key.
    images: HashMap<String, (usize, Ref)>,
    pages: Vec<Content>,
}

impl Doc {
    fn new(paper: Paper) -> Self {
        let (width, height) = paper.size();
        Self {
            pdf: Pdf::new(),
            last_ref: 0,
            width,
            height,
            fonts: [
                Font::new(FONT_REGULAR, b"F1", b"SDPAAA+NotoSans-Regular"),
                Font::new(FONT_MEDIUM, b"F2", b"SDPAAB+NotoSans-Medium"),
            ],
            images: HashMap::new(),
            pages: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Ref {
        self.last_ref += 1;
        Ref::new(self.last_ref)
    }

    fn new_page(&mut self) {
        self.pages.push(Content::new());
    }

    fn content(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page")
    }

    /// Draws the picture of the card `key`, embedded once however many times it is drawn.
    fn image(&mut self, key: &str, jpeg: &Jpeg, x: f32, y: f32, w: f32, h: f32) {
        let index = match self.images.get(key) {
            Some(&(index, _)) => index,
            None => {
                let id = self.alloc();
                let mut xobject = self.pdf.image_xobject(id, &jpeg.data);
                xobject.filter(Filter::DctDecode);
                xobject.width(jpeg.width as i32);
                xobject.height(jpeg.height as i32);
                xobject.color_space().device_rgb();
                xobject.bits_per_component(8);
                xobject.finish();
                let index = self.images.len();
                self.images.insert(key.to_string(), (index, id));
                index
            }
        };
        let bottom = self.height - y - h;
        let name = format!("Im{index}");
        let content = self.content();
        content.save_state();
        content.transform([w, 0.0, 0.0, h, x, bottom]);
        content.x_object(Name(name.as_bytes()));
        content.restore_state();
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: [f32; 3]) {
        let (y1, y2) = (self.height - y1, self.height - y2);
        let content = self.content();
        content.set_line_width(width);
        content.set_stroke_rgb(color[0], color[1], color[2]);
        content.move_to(x1, y1);
        content.line_to(x2, y2);
        content.stroke();
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: [f32; 3]) {
        let bottom = self.height - y - h;
        let content = self.content();
        content.set_line_width(width);
        content.set_stroke_rgb(color[0], color[1], color[2]);
        content.rect(x, bottom, w, h);
        content.stroke();
    }

    fn font(&self, bold: bool) -> &Font {
        &self.fonts[usize::from(bold)]
    }

    /// Greedy word wrap at `width`.
    fn wrap(&self, block: &Block, width: f32) -> Vec<Line> {
        let size = block.size;
        let mut lines = Vec::new();
        let mut line: Line = Vec::new();
        let mut x = 0.0;
        let mut space: Option<Style> = None;
        let push = |line: &mut Line, x: f32, text: &str, style: Style| match line.last_mut() {
            Some((_, last, s)) if *s == style => last.push_str(text),
            _ => line.push((x, text.to_string(), style)),
        };
        for run in &block.runs {
            for (word, is_space) in words(&run.text) {
                if is_space {
                    if !line.is_empty() {
                        space = Some(run.style);
                    }
                    continue;
                }
                let w = self.font(run.style.bold).width(word, size);
                let mut sw = space.map_or(0.0, |s| self.font(s.bold).width(" ", size));
                if !line.is_empty() && x + sw + w > width {
                    lines.push(std::mem::take(&mut line));
                    x = 0.0;
                    space = None;
                    sw = 0.0;
                }
                if let Some(s) = space.take() {
                    push(&mut line, x, " ", s);
                    x += sw;
                }
                push(&mut line, x, word, run.style);
                x += w;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    fn leading(size: f32) -> f32 {
        size * 1.3
    }

    fn paragraph_height(&self, block: &Block, width: f32) -> f32 {
        block.space_before + self.wrap(block, width).len() as f32 * Self::leading(block.size)
    }

    /// Draws the block from `top` down; lines past `bottom` are dropped. Returns the next top.
    fn paragraph(&mut self, block: &Block, x: f32, top: f32, width: f32, bottom: f32) -> f32 {
        let size = block.size;
        let leading = Self::leading(size);
        let mut top = top + block.space_before;
        for line in self.wrap(block, width) {
            if top + leading > bottom {
                break;
            }
            // Baseline: ascent is about 1.07 em for Noto Sans, centred in the leading.
            let baseline = self.height - (top + (leading - size) / 2.0 + size * 0.95);
            for (dx, text, style) in line {
                let font = &mut self.fonts[usize::from(style.bold)];
                let glyphs = font.encode(&text);
                let resource = font.resource;
                let content = self.content();
                content.begin_text();
                content.set_font(Name(resource), size);
                content.set_fill_rgb(style.color[0], style.color[1], style.color[2]);
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x + dx, baseline]);
                content.show(Str(&glyphs));
                content.end_text();
            }
            top += leading;
        }
        top
    }

    fn finish(mut self, title: &str) -> Result<Vec<u8>> {
        let catalog = self.alloc();
        let tree = self.alloc();
        let info = self.alloc();
        let mut fonts = Vec::new();
        for i in 0..self.fonts.len() {
            if !self.fonts[i].glyphs.is_empty() {
                let refs = [(); 5].map(|_| self.alloc());
                self.fonts[i].write(&mut self.pdf, refs)?;
                fonts.push((self.fonts[i].resource, refs[0]));
            }
        }
        let pages = std::mem::take(&mut self.pages);
        let mut kids = Vec::new();
        for content in pages {
            let (page_id, content_id) = (self.alloc(), self.alloc());
            kids.push(page_id);
            self.pdf
                .stream(content_id, &deflate(&content.finish()))
                .filter(Filter::FlateDecode);
            let mut page = self.pdf.page(page_id);
            page.media_box(Rect::new(0.0, 0.0, self.width, self.height))
                .parent(tree)
                .contents(content_id);
            let mut resources = page.resources();
            let mut font_dict = resources.fonts();
            for &(name, id) in &fonts {
                font_dict.pair(Name(name), id);
            }
            font_dict.finish();
            let mut x_objects = resources.x_objects();
            for &(index, id) in self.images.values() {
                x_objects.pair(Name(format!("Im{index}").as_bytes()), id);
            }
        }
        let count = kids.len() as i32;
        self.pdf.pages(tree).kids(kids).count(count);
        self.pdf.catalog(catalog).pages(tree);
        self.pdf
            .document_info(info)
            .title(TextStr(title))
            .creator(TextStr("SimuDeckParser"));
        Ok(self.pdf.finish())
    }
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

    #[test]
    fn wrapping() {
        let doc = Doc::new(Paper::A4);
        let b = block(vec![Run::new("aaa bbb ccc", false, BLACK)], 10.0);
        let width = doc.font(false).width("aaa bbb", 10.0) + 1.0;
        let lines: Vec<Vec<String>> = doc
            .wrap(&b, width)
            .into_iter()
            .map(|l| l.into_iter().map(|(_, t, _)| t).collect())
            .collect();
        assert_eq!(lines, [vec!["aaa bbb"], vec!["ccc"]]);
    }
}
