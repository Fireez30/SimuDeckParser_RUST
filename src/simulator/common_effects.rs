//! `StreamingAssets/CommonEffects_USER_REFERENCE.txt` (formerly `CommonEffects(copy).txt`):
//! reusable effects referenced by cards as `*EffectName(param1,param2)`. Each has a script
//! (what the simulator runs) and a text (what players read).

use std::collections::BTreeMap;
use std::path::Path;

/// Effect name (with its parameter list, e.g. `Brainstorm($X)`) -> effect text, and -> script.
/// Ordered, because prefix matching takes the first matching key.
#[derive(Debug, Clone, Default)]
pub struct CommonEffects {
    pub text: BTreeMap<String, String>,
    pub script: BTreeMap<String, String>,
}

pub fn load_common_effects(path: &Path) -> CommonEffects {
    match std::fs::read(path) {
        Ok(bytes) => parse_common_effects(&crate::text::decode_text(&bytes)),
        Err(_) => CommonEffects::default(),
    }
}

pub fn parse_common_effects(content: &str) -> CommonEffects {
    let mut effects = CommonEffects::default();
    let mut key = String::new();
    let mut value = String::new();
    // The script: the lines between `Define:` and the first `Text` line, one per line.
    let mut script = String::new();
    let mut reading_value = false;

    let mut flush = |key: &mut String, value: &mut String, script: &mut String| {
        if !key.is_empty() && !script.is_empty() {
            effects.script.insert(key.clone(), std::mem::take(script));
        }
        if !key.is_empty() && !value.is_empty() {
            effects
                .text
                .insert(std::mem::take(key), std::mem::take(value));
        }
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || line.starts_with("//") || trimmed == "Quick" {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Define:") {
            flush(&mut key, &mut value, &mut script);
            key = rest.trim().to_string();
            value.clear();
            script.clear();
            reading_value = false;
        } else if key.is_empty() {
            continue;
        } else if let Some(rest) = trimmed.strip_prefix("Text ") {
            reading_value = true;
            value = rest.trim().to_string();
        } else if reading_value
            && !["Tag ", "Act:", "Auto:", "Cont:"]
                .iter()
                .any(|p| trimmed.starts_with(p))
        {
            value.push_str(trimmed);
        } else if !reading_value && !trimmed.starts_with("Tag ") {
            script.push_str(trimmed);
            script.push('\n');
        }
    }
    // The original dropped the last definition of the file.
    flush(&mut key, &mut value, &mut script);
    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_definitions() {
        let fx = parse_common_effects(
            "// comment\n\
             Define: Brainstorm($X)\n\
             Quick\n\
             Auto: something\n\
             Text [A] BRAINSTORM put the top $X cards\n\
             into your waiting room.\n\
             Tag Brainstorm\n\
             \n\
             Define: Encore\n\
             Text [A] ENCORE [Put 1 character from your hand into your waiting room]\n",
        );
        assert_eq!(fx.text.len(), 2);
        assert_eq!(
            fx.text["Brainstorm($X)"],
            "[A] BRAINSTORM put the top $X cardsinto your waiting room."
        );
        assert_eq!(
            fx.text["Encore"],
            "[A] ENCORE [Put 1 character from your hand into your waiting room]"
        );
        assert_eq!(fx.script["Brainstorm($X)"], "Auto: something\n");
        assert!(!fx.script.contains_key("Encore"));
    }
}
