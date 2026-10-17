//! Fonts embedded in the PDFs. Only the characters a PDF uses are kept ("subsetting"), which
//! keeps the files small.

use std::collections::BTreeMap;
use std::io::Write as _;

use anyhow::Result;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str};
use subsetter::GlyphRemapper;

/// An embedded font (subset to the glyphs used), addressed by glyph id.
pub(super) struct Font {
    pub(super) data: &'static [u8],
    pub(super) face: ttf_parser::Face<'static>,
    /// Resource name and PostScript name (with a subset tag).
    pub(super) resource: &'static [u8],
    pub(super) base: &'static [u8],
    pub(super) remapper: GlyphRemapper,
    /// Subset glyph id -> width (thousandths of an em) and the character it draws.
    pub(super) glyphs: BTreeMap<u16, (f32, char)>,
}

impl Font {
    pub(super) fn new(data: &'static [u8], resource: &'static [u8], base: &'static [u8]) -> Self {
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

    pub(super) fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.glyph(c).1).sum::<f32>() * size
    }

    /// Glyph ids for a `show` operation (Identity-H: two bytes each).
    pub(super) fn encode(&mut self, text: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() * 2);
        for c in text.chars().filter(|c| !c.is_control()) {
            let (id, advance) = self.glyph(c);
            let cid = self.remapper.remap(id.0);
            self.glyphs.entry(cid).or_insert((advance * 1000.0, c));
            out.extend(cid.to_be_bytes());
        }
        out
    }

    pub(super) fn write(&self, pdf: &mut Pdf, refs: [Ref; 5]) -> Result<()> {
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

pub(super) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}
