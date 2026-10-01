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

/// A run of card text, after reading the simulator's rich-text tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rich<'a> {
    Text {
        text: &'a str,
        bold: bool,
        italic: bool,
        /// Inside a `<color=…>` tag (always red in the card data).
        highlight: bool,
        strike: bool,
    },
    /// `<sprite name="1cost"/>`: an inline icon.
    Sprite(&'a str),
}

/// Splits a line of card text on its Unity rich-text tags (`<b>`, `<i>`, `<s>`, `<color=…>`,
/// `<sprite name="…"/>`). Anything else in angle brackets, such as the trait `<Magic>`, is text.
pub fn parse_rich(line: &str) -> Vec<Rich<'_>> {
    // Nesting depth of bold, italic, highlight and strike.
    let mut depth = [0u32; 4];
    let run = |text, depth: [u32; 4]| Rich::Text {
        text,
        bold: depth[0] > 0,
        italic: depth[1] > 0,
        highlight: depth[2] > 0,
        strike: depth[3] > 0,
    };
    let mut out = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while let Some(offset) = line[i..].find('<') {
        let start = i + offset;
        let Some(len) = line[start..].find('>') else {
            break;
        };
        let tag = &line[start + 1..start + len];
        let lower = tag.trim().to_ascii_lowercase();
        // (style index, opening?)
        let style = match lower.as_str() {
            "b" => Some((0, true)),
            "/b" => Some((0, false)),
            "i" => Some((1, true)),
            "/i" => Some((1, false)),
            "/color" => Some((2, false)),
            "s" => Some((3, true)),
            "/s" => Some((3, false)),
            _ if lower.starts_with("color") => Some((2, true)),
            _ => None,
        };
        let sprite = lower
            .strip_prefix("sprite")
            .filter(|rest| rest.starts_with([' ', '=']))
            .map(|_| sprite_name(tag));
        if style.is_none() && sprite.is_none() {
            i = start + 1;
            continue;
        }
        if start > text_start {
            out.push(run(&line[text_start..start], depth));
        }
        if let Some(name) = sprite {
            out.push(Rich::Sprite(name));
        }
        if let Some((index, open)) = style {
            depth[index] = if open {
                depth[index] + 1
            } else {
                depth[index].saturating_sub(1)
            };
        }
        i = start + len + 1;
        text_start = i;
    }
    if text_start < line.len() {
        out.push(run(&line[text_start..], depth));
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
        let plain = |text| Rich::Text {
            text,
            bold: false,
            italic: false,
            highlight: false,
            strike: false,
        };
        assert_eq!(
            parse_rich(r#"Auto: [<sprite name="1cost"/>] When <Magic> is <b>here</b>"#),
            vec![
                plain("Auto: ["),
                Rich::Sprite("1cost"),
                plain("] When <Magic> is "),
                Rich::Text {
                    text: "here",
                    bold: true,
                    italic: false,
                    highlight: false,
                    strike: false,
                },
            ]
        );
        assert_eq!(
            parse_rich("<color=red>x</color> y <hololive"),
            vec![
                Rich::Text {
                    text: "x",
                    bold: false,
                    italic: false,
                    highlight: true,
                    strike: false,
                },
                plain(" y <hololive"),
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
    fn stem() {
        assert_eq!(image_stem(" KS/W49-E073 "), "KS_W49_E073");
    }
}
