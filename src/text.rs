//! Text helpers: decoding the simulator's files, reading the markup of card text, and finding
//! the card names quoted in it.

use crate::model::Trigger;

/// Strips trailing ASCII letters (`"ABC/W1-001SP"` -> `"ABC/W1-001"`).
pub fn remove_trailing_alphas(input: &str) -> &str {
    input.trim_end_matches(|c: char| c.is_ascii_alphabetic())
}

/// Case-insensitive substring search; an empty needle always matches.
pub fn contains_ignore_case(needle: &str, haystack: &str) -> bool {
    needle.is_empty() || haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Card code -> image file stem used by the simulator (`ABC/W1-001` -> `ABC_W1_001`).
pub fn image_stem(code: &str) -> String {
    code.trim().replace(['/', '-'], "_")
}

/// Decodes a simulator text file: UTF-8 (BOM optional) or UTF-16 (with a BOM, or detected by its
/// NUL bytes, as some card files are saved by Windows editors).
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], little: bool| {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if little {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => {
            let sample = &bytes[..bytes.len().min(512)];
            let odd_nuls = sample
                .iter()
                .skip(1)
                .step_by(2)
                .filter(|&&b| b == 0)
                .count();
            let even_nuls = sample.iter().step_by(2).filter(|&&b| b == 0).count();
            let half = sample.len() / 4;
            if sample.len() >= 8 && odd_nuls > half {
                utf16(bytes, true)
            } else if sample.len() >= 8 && even_nuls > half {
                utf16(bytes, false)
            } else {
                String::from_utf8_lossy(bytes).into_owned()
            }
        }
    }
}

/// How a run of card text is styled by the simulator's rich-text tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    /// Inside a `<color=…>` tag (always red in the card data).
    pub highlight: bool,
    pub strike: bool,
}

/// A piece of card text, after reading the simulator's rich-text tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rich<'a> {
    Text(&'a str, TextStyle),
    /// `<sprite name="1cost"/>`: an inline icon.
    Sprite(&'a str),
}

/// How many `<b>`, `<i>`… tags are open at a point of the text. Tags can nest (`<b>a<b>b</b>
/// c</b>`), so they are counted rather than switched on and off.
#[derive(Default)]
struct OpenTags {
    bold: u32,
    italic: u32,
    highlight: u32,
    strike: u32,
}

impl OpenTags {
    fn style(&self) -> TextStyle {
        TextStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            highlight: self.highlight > 0,
            strike: self.strike > 0,
        }
    }

    /// Applies a style tag (`b`, `/b`, `color=red`…). Returns `false` when `tag` is not one.
    fn apply(&mut self, tag: &str) -> bool {
        let (counter, opening) = match tag {
            "b" => (&mut self.bold, true),
            "/b" => (&mut self.bold, false),
            "i" => (&mut self.italic, true),
            "/i" => (&mut self.italic, false),
            "s" => (&mut self.strike, true),
            "/s" => (&mut self.strike, false),
            "/color" => (&mut self.highlight, false),
            _ if tag.starts_with("color") => (&mut self.highlight, true),
            _ => return false,
        };
        if opening {
            *counter += 1;
        } else {
            *counter = counter.saturating_sub(1);
        }
        true
    }
}

/// Splits a line of card text on its Unity rich-text tags (`<b>`, `<i>`, `<s>`, `<color=…>`,
/// `<sprite name="…"/>`). Anything else in angle brackets, such as the trait `<Magic>`, is text.
pub fn parse_rich(line: &str) -> Vec<Rich<'_>> {
    let mut open = OpenTags::default();
    let mut out = Vec::new();
    // Start of the text not yet pushed to `out`, and where to look for the next `<`.
    let mut text_start = 0;
    let mut search_from = 0;
    while let Some(offset) = line[search_from..].find('<') {
        let start = search_from + offset;
        let Some(len) = line[start..].find('>') else {
            break;
        };
        let end = start + len + 1;
        let tag = &line[start + 1..end - 1];
        let lower = tag.trim().to_ascii_lowercase();
        let is_sprite = lower
            .strip_prefix("sprite")
            .is_some_and(|rest| rest.starts_with([' ', '=']));

        // The text before the tag keeps the style that was active before it.
        let style_before = open.style();
        let is_style = !is_sprite && open.apply(&lower);
        if !is_sprite && !is_style {
            // Not a tag: `<Magic>` stays in the text.
            search_from = start + 1;
            continue;
        }
        if start > text_start {
            out.push(Rich::Text(&line[text_start..start], style_before));
        }
        if is_sprite {
            out.push(Rich::Sprite(sprite_name(tag)));
        }
        text_start = end;
        search_from = end;
    }
    if text_start < line.len() {
        out.push(Rich::Text(&line[text_start..], open.style()));
    }
    out
}

/// `sprite name="1cost"/` -> `1cost`.
fn sprite_name(tag: &str) -> &str {
    let value = tag.split_once('=').map_or("", |(_, v)| v);
    value
        .trim()
        .trim_end_matches('/')
        .trim()
        .trim_matches('"')
        .trim()
}

/// Keywords of card text, spelled out in bold (`ALARM` -> **Alarm**).
pub const KEYWORDS: &[(&str, &str)] = &[
    ("ALARM", "Alarm"),
    ("ACCELERATE", "Accelerate"),
    ("ASSIST", "Assist"),
    ("BACKUP", "Backup"),
    ("BOND", "Bond"),
    ("BRAINSTORM", "Brainstorm"),
    ("CHANGE", "Change"),
    ("ENCORE", "Encore"),
    ("EXPERIENCE", "Experience"),
    ("GREAT PERFORMANCE", "Great Performance"),
    ("MEMORY", "Memory"),
    ("SHIFT", "Shift"),
];

/// Trigger drawn by a `<sprite name="mini_…"/>` of card text.
pub fn sprite_trigger(name: &str) -> Option<Trigger> {
    match name {
        "mini_soul" => Some(Trigger::Soul),
        "mini_choice" => Some(Trigger::Choice),
        "mini_shot" => Some(Trigger::Burn),
        "mini_bounce" => Some(Trigger::Wind),
        "mini_standby" => Some(Trigger::Standby),
        "mini_door" => Some(Trigger::Salvage),
        "mini_gate" => Some(Trigger::Pant),
        "mini_book" => Some(Trigger::Book),
        "mini_bar" => Some(Trigger::Bar),
        "mini_bag" => Some(Trigger::Bag),
        _ => None,
    }
}

/// Stock paid by a `<sprite name="2cost"/>`.
pub fn sprite_cost(name: &str) -> Option<u32> {
    name.strip_suffix("cost")?.parse().ok()
}

/// Word for the other sprites: card states, turn counts, keywords (`"reverse"` -> `Reverse`).
pub fn sprite_word(name: &str) -> String {
    match name {
        // The counter (hand) icon of events that can be played during battle.
        "backup" => "Counter".to_string(),
        _ => match name.strip_suffix("turn") {
            Some(n) if n.parse::<u32>().is_ok() => format!("{n} turn"),
            _ => {
                let word = name.strip_prefix("mini_").unwrap_or(name);
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            }
        },
    }
}

/// Splits text on quoted card names: `(span, name)` pairs, where quoted spans keep their quote
/// marks and `name` is what is between them. Any quote marks pair up (`"…"`, `'…'`, `“…”`,
/// `「…」`), since names hold quotes themselves (`'"Significance of…" Aqua'`); `is_name` picks
/// the pairs that enclose a name, the longest one first.
pub fn split_names(text: &str, is_name: impl Fn(&str) -> bool) -> Vec<(&str, Option<&str>)> {
    const QUOTES: &[char] = &['"', '\'', '“', '”', '‘', '’', '「', '」'];
    let quotes: Vec<(usize, usize)> = text
        .char_indices()
        .filter(|(_, c)| QUOTES.contains(c))
        .map(|(i, c)| (i, i + c.len_utf8()))
        .collect();
    let mut out = Vec::new();
    let mut done = 0;
    let mut x = 0;
    while x < quotes.len() {
        let (start, inner) = quotes[x];
        let close = quotes[x + 1..]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (end, _))| *end > inner && is_name(&text[inner..*end]));
        let Some((k, &(name_end, end))) = close else {
            x += 1;
            continue;
        };
        if start > done {
            out.push((&text[done..start], None));
        }
        out.push((&text[start..end], Some(&text[inner..name_end])));
        done = end;
        x += k + 2;
    }
    if done < text.len() {
        out.push((&text[done..], None));
    }
    out
}

/// `%20` and the like decoded (`My%20Deck` -> `My Deck`); invalid escapes are kept as they are.
/// Works byte by byte so that multi-byte UTF-8 characters (`%C3%A9` -> `é`) survive.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| text.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_alphas() {
        assert_eq!(remove_trailing_alphas("KS/W49-E073SP"), "KS/W49-E073");
        assert_eq!(remove_trailing_alphas("KS/W49-E073"), "KS/W49-E073");
        assert_eq!(remove_trailing_alphas("abc"), "");
    }

    #[test]
    fn ignore_case() {
        assert!(contains_ignore_case("", "anything"));
        assert!(contains_ignore_case("miku", "Hatsune MIKU"));
        assert!(!contains_ignore_case("rin", "Miku"));
    }

    #[test]
    fn decoding() {
        let le: Vec<u8> = "Name é".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let be: Vec<u8> = "Name é".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(decode_text(&[&[0xFF, 0xFE][..], &le].concat()), "Name é");
        assert_eq!(decode_text(&[&[0xFE, 0xFF][..], &be].concat()), "Name é");
        assert_eq!(decode_text(&le), "Name é");
        assert_eq!(decode_text("\u{feff}Name é".as_bytes()), "Name é");
        assert_eq!(decode_text("Name é".as_bytes()), "Name é");
    }

    #[test]
    fn rich_text() {
        let plain = |text| Rich::Text(text, TextStyle::default());
        let bold = TextStyle {
            bold: true,
            ..TextStyle::default()
        };
        let red = TextStyle {
            highlight: true,
            ..TextStyle::default()
        };
        assert_eq!(
            parse_rich(r#"Auto: [<sprite name="1cost"/>] When <Magic> is <b>here</b>"#),
            vec![
                plain("Auto: ["),
                Rich::Sprite("1cost"),
                plain("] When <Magic> is "),
                Rich::Text("here", bold),
            ]
        );
        assert_eq!(
            parse_rich("<color=red>x</color> y <hololive"),
            vec![Rich::Text("x", red), plain(" y <hololive")]
        );
        assert_eq!(
            parse_rich("<b>a<b>b</b>c</b>d"),
            vec![
                Rich::Text("a", bold),
                Rich::Text("b", bold),
                Rich::Text("c", bold),
                plain("d"),
            ]
        );
        assert_eq!(
            parse_rich("<sprite name=reverse>"),
            vec![Rich::Sprite("reverse")]
        );
        assert_eq!(parse_rich(""), vec![]);
    }

    #[test]
    fn quoted_names() {
        let names = [
            "Aqua",
            "Megumin",
            r#""Significance of Killing Snow Sprites?" Aqua"#,
        ];
        let split = |text| split_names(text, |n| names.contains(&n));
        assert_eq!(
            split(r#"your "Aqua" and 「Megumin」's "Darkness" get "#),
            vec![
                ("your ", None),
                (r#""Aqua""#, Some("Aqua")),
                (" and ", None),
                ("「Megumin」", Some("Megumin")),
                (r#"'s "Darkness" get "#, None),
            ]
        );
        assert_eq!(
            split(r#"If '"Significance of Killing Snow Sprites?" Aqua' is in"#),
            vec![
                ("If ", None),
                (
                    r#"'"Significance of Killing Snow Sprites?" Aqua'"#,
                    Some(names[2])
                ),
                (" is in", None),
            ]
        );
        assert_eq!(split("“Aqua”"), vec![("“Aqua”", Some("Aqua"))]);
        assert_eq!(split(r#"a "" b "open"#), vec![(r#"a "" b "open"#, None)]);
        assert_eq!(split(""), vec![]);
    }

    #[test]
    fn sprites() {
        assert_eq!(sprite_cost("2cost"), Some(2));
        assert_eq!(sprite_cost("xcost"), None);
        assert_eq!(sprite_trigger("mini_door"), Some(Trigger::Salvage));
        assert_eq!(sprite_word("backup"), "Counter");
        assert_eq!(sprite_word("2turn"), "2 turn");
        assert_eq!(sprite_word("mini_clock"), "Clock");
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("A%20B%C3%A9"), "A Bé");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn stem() {
        assert_eq!(image_stem(" KS/W49-E073 "), "KS_W49_E073");
    }
}
