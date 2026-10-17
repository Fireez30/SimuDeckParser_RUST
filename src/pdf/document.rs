//! A PDF being drawn: pages, lines, pictures, and paragraphs of wrapped text.
//!
//! Positions are in points (1/72 inch) from the top-left corner of the page; PDFs count from the
//! bottom-left, so [`Doc`] flips the y axis when it writes.

use std::collections::HashMap;

use anyhow::Result;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use super::Paper;
use super::artwork::Jpeg;
use super::font::{Font, deflate};
use crate::fonts::{NOTO_SANS_MEDIUM, NOTO_SANS_REGULAR};

/// A colour, as red, green and blue between 0 and 1.
pub(super) type Rgb = [f32; 3];

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Style {
    pub(super) bold: bool,
    pub(super) color: Rgb,
}

/// A piece of text in one style.
pub(super) struct Run {
    pub(super) text: String,
    pub(super) style: Style,
}

impl Run {
    pub(super) fn new(text: &str, bold: bool, color: Rgb) -> Self {
        Self {
            text: text.to_string(),
            style: Style { bold, color },
        }
    }
}

/// A paragraph: runs of text wrapped together at one size.
pub(super) struct Block {
    pub(super) runs: Vec<Run>,
    pub(super) size: f32,
    pub(super) space_before: f32,
}

pub(super) fn block(runs: Vec<Run>, size: f32) -> Block {
    Block {
        runs,
        size,
        space_before: 0.0,
    }
}

/// A wrapped line: pieces of text at their x offset.
pub(super) type Line = Vec<(f32, String, Style)>;

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

/// A PDF being drawn, in points from the top-left corner of the page.
pub(super) struct Doc {
    pdf: Pdf,
    last_ref: i32,
    pub(super) width: f32,
    pub(super) height: f32,
    fonts: [Font; 2],
    /// Image XObjects, named `Im{index}`, by card key.
    images: HashMap<String, (usize, Ref)>,
    pages: Vec<Content>,
}

impl Doc {
    pub(super) fn new(paper: Paper) -> Self {
        let (width, height) = paper.size();
        Self {
            pdf: Pdf::new(),
            last_ref: 0,
            width,
            height,
            fonts: [
                Font::new(NOTO_SANS_REGULAR, b"F1", b"SDPAAA+NotoSans-Regular"),
                Font::new(NOTO_SANS_MEDIUM, b"F2", b"SDPAAB+NotoSans-Medium"),
            ],
            images: HashMap::new(),
            pages: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Ref {
        self.last_ref += 1;
        Ref::new(self.last_ref)
    }

    pub(super) fn new_page(&mut self) {
        self.pages.push(Content::new());
    }

    fn content(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page")
    }

    /// Draws the picture of the card `key`, embedded once however many times it is drawn.
    pub(super) fn image(&mut self, key: &str, jpeg: &Jpeg, x: f32, y: f32, w: f32, h: f32) {
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

    pub(super) fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Rgb) {
        let (y1, y2) = (self.height - y1, self.height - y2);
        let content = self.content();
        content.set_line_width(width);
        content.set_stroke_rgb(color[0], color[1], color[2]);
        content.move_to(x1, y1);
        content.line_to(x2, y2);
        content.stroke();
    }

    pub(super) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Rgb) {
        let bottom = self.height - y - h;
        let content = self.content();
        content.set_line_width(width);
        content.set_stroke_rgb(color[0], color[1], color[2]);
        content.rect(x, bottom, w, h);
        content.stroke();
    }

    pub(super) fn font(&self, bold: bool) -> &Font {
        &self.fonts[usize::from(bold)]
    }

    /// Greedy word wrap at `width`.
    pub(super) fn wrap(&self, block: &Block, width: f32) -> Vec<Line> {
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

    pub(super) fn paragraph_height(&self, block: &Block, width: f32) -> f32 {
        block.space_before + self.wrap(block, width).len() as f32 * Self::leading(block.size)
    }

    /// Draws the block from `top` down; lines past `bottom` are dropped. Returns the next top.
    pub(super) fn paragraph(
        &mut self,
        block: &Block,
        x: f32,
        top: f32,
        width: f32,
        bottom: f32,
    ) -> f32 {
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

    pub(super) fn finish(mut self, title: &str) -> Result<Vec<u8>> {
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
    use crate::pdf::BLACK;

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
