use regex::Regex;
use std::sync::LazyLock;

static ANCHOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\{#([a-zA-Z0-9_-]+)\}\s*$").unwrap());

/// Strip an anchor like `{#some-id}` from the end of a string.
/// Returns (cleaned_text, Some(anchor_id)) or (original_text, None).
pub fn strip_anchor(text: &str) -> (String, Option<String>) {
    if let Some(caps) = ANCHOR_RE.captures(text) {
        let anchor_id = caps[1].to_string();
        let cleaned = ANCHOR_RE.replace(text, "").to_string();
        (cleaned, Some(anchor_id))
    } else {
        (text.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_anchor() {
        let (text, anchor) = strip_anchor("Definitions {#definitions}");
        assert_eq!(text, "Definitions");
        assert_eq!(anchor, Some("definitions".to_string()));
    }

    #[test]
    fn test_no_anchor() {
        let (text, anchor) = strip_anchor("Just some text");
        assert_eq!(text, "Just some text");
        assert_eq!(anchor, None);
    }

    #[test]
    fn test_anchor_with_text_before() {
        let (text, anchor) = strip_anchor("The Employer shall pay. {#payment-timeframe}");
        assert_eq!(text, "The Employer shall pay.");
        assert_eq!(anchor, Some("payment-timeframe".to_string()));
    }
}
