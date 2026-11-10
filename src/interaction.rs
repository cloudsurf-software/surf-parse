//! The interaction grammar (0.47.0) — how a source says what a person does
//! with an app, on every input and every screen.
//!
//! Six attributes ride on any block (they are captured by
//! [`crate::block_meta`] next to `id=` and `label=`), and three of them ride
//! on a toolbar `- button[…]` item too:
//!
//! | Attribute | Where | Value |
//! |---|---|---|
//! | `surface=` | any block | the surface's ledger id, `kind.name` (`sheet.invite`, `panel.tasks`) |
//! | `key=` | any block, toolbar buttons | one key chord (`mod+k`) that triggers the item |
//! | `focus=` | any block, toolbar buttons | the item's place in the keyboard order (`1`, `2` …) or `none` |
//! | `reveal=` | any block, toolbar buttons | `always` (the default) or `hover` |
//! | `native=` | any block | one of the closed native-only list ([`NATIVE_SEATS`]) |
//! | `pane=` | a child of a two-pane `::app-shell` | `start` or `end` |
//!
//! And three on `::app-shell` alone:
//!
//! | Attribute | Value |
//! |---|---|
//! | `shortcuts=` | whitespace-separated `chord=target` pairs (`mod+1=#surfy mod+k=openSearch`) |
//! | `panes=` | `one` (the default) or `two` |
//! | `panes-at=` | the smallest width class that shows two panes: `tablet` (the default) or `desktop` |
//!
//! ## The key chord
//!
//! Lower-case words joined by `+`: any of the modifiers `mod`, `ctrl`, `alt`,
//! `shift` (each at most once), then exactly one key. `mod` is the platform's
//! command modifier — Command on Apple platforms, Control everywhere else —
//! so `mod+k` is one chord for six clients. `ctrl` is the Control key on
//! every platform, `alt` is Option on Apple platforms and Alt elsewhere. A
//! key is a letter `a`–`z`, a digit `0`–`9`, `f1`–`f12`, or one of
//! [`NAMED_KEYS`]. Platform words (`cmd`, `command`, `meta`, `win`, `super`,
//! `option`, `opt`, `control`) are refused with the neutral word to use.
//! Input is case-insensitive; the chord is normalised to lower case with the
//! modifiers in the order `mod`, `ctrl`, `alt`, `shift`.
//!
//! ## Reveal and focus
//!
//! `reveal=hover` says an affordance is drawn only while a pointer is over
//! its row. A player MUST also draw it while the row holds keyboard focus and
//! on a touch screen — but the source has to give it a keyboard path first:
//! a `focus=` place (a number) or a `key=` chord. A hover reveal with neither
//! (or with `focus=none`) is lint error L052: it would be reachable only on
//! hover.
//!
//! `focus=<n>` (1–9999) orders the items of one shell, lowest first; items
//! with the same number keep source order, and items without a number follow
//! the numbered ones in source order. `focus=none` takes an item out of the
//! keyboard order (decoration, or a control reached by its `key=` alone).
//!
//! ## Two panes
//!
//! `::app-shell[panes=two]` declares that from `panes-at=` up the shell
//! shows two panes side by side, and each direct child that belongs to one
//! says which with `pane=start` or `pane=end` (start is the reading-order
//! first side). A player MAY put the split where the display has a physical
//! seam; the grammar names only the layout, never a device.

use crate::types::{AttrValue, Attrs};

/// The closed native-only list (D-APP-11): the things a player draws with
/// its platform rather than from the source. `native=` takes exactly these;
/// anything else is lint error L053.
pub const NATIVE_SEATS: &[&str] = &[
    "voice-mode",
    "camera",
    "terminal",
    "device-pairing",
    "share-sheet",
    "platform-sign-in",
    "store-billing",
    "menu-bar",
];

/// The modifiers of a key chord, in normalised order.
pub const MODIFIERS: &[&str] = &["mod", "ctrl", "alt", "shift"];

/// The named (non-letter, non-digit, non-function) keys a chord may end in.
pub const NAMED_KEYS: &[&str] = &[
    "enter", "escape", "tab", "space", "backspace", "delete", "up", "down", "left", "right",
    "home", "end", "pageup", "pagedown", "comma", "period", "slash", "backslash", "semicolon",
    "quote", "backquote", "minus", "equal", "bracketleft", "bracketright",
];

/// `reveal=` values.
pub const REVEALS: &[&str] = &["always", "hover"];
/// `pane=` values.
pub const PANES: &[&str] = &["start", "end"];
/// `panes=` values.
pub const PANE_COUNTS: &[&str] = &["one", "two"];
/// `panes-at=` values.
pub const PANES_AT: &[&str] = &["tablet", "desktop"];

/// Why a key chord did not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChordError {
    /// Nothing was written.
    Empty,
    /// A platform word where a neutral one belongs (`cmd` → `mod`).
    PlatformWord { word: String, use_instead: &'static str },
    /// A modifier written twice.
    RepeatedModifier(String),
    /// Only modifiers, no key.
    NoKey,
    /// More than one non-modifier key.
    TwoKeys(String, String),
    /// A word that is neither a modifier nor a known key.
    UnknownKey(String),
}

impl std::fmt::Display for ChordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChordError::Empty => write!(f, "the chord is empty"),
            ChordError::PlatformWord { word, use_instead } => {
                write!(f, "'{word}' names one platform's key — write '{use_instead}'")
            }
            ChordError::RepeatedModifier(m) => write!(f, "'{m}' is written twice"),
            ChordError::NoKey => write!(f, "it names modifiers but no key"),
            ChordError::TwoKeys(a, b) => write!(f, "it names two keys, '{a}' and '{b}'"),
            ChordError::UnknownKey(k) => write!(
                f,
                "'{k}' is not a key — use a letter, a digit, f1–f12 or a named key (enter, escape, comma …)"
            ),
        }
    }
}

fn platform_word(word: &str) -> Option<&'static str> {
    match word {
        "cmd" | "command" | "meta" | "win" | "super" | "⌘" => Some("mod"),
        "option" | "opt" | "⌥" => Some("alt"),
        "control" | "⌃" => Some("ctrl"),
        _ => None,
    }
}

fn is_key(word: &str) -> bool {
    let mut chars = word.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_lowercase() || c.is_ascii_digit(),
        _ => {
            let function_key = word.strip_prefix('f').is_some_and(|n| {
                !n.starts_with('0')
                    && n.bytes().all(|b| b.is_ascii_digit())
                    && n.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))
            });
            function_key || NAMED_KEYS.contains(&word)
        }
    }
}

/// Read one key chord, returning its normalised spelling (`Mod+Shift+K` →
/// `mod+shift+k`; `shift+mod+k` → `mod+shift+k`).
pub fn parse_chord(raw: &str) -> Result<String, ChordError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ChordError::Empty);
    }
    let lower = raw.to_lowercase();
    let mut mods = [false; 4];
    let mut key: Option<String> = None;
    for word in lower.split('+') {
        let word = word.trim();
        if word.is_empty() {
            return Err(ChordError::UnknownKey(raw.to_string()));
        }
        if let Some(use_instead) = platform_word(word) {
            return Err(ChordError::PlatformWord { word: word.to_string(), use_instead });
        }
        if let Some(i) = MODIFIERS.iter().position(|m| *m == word) {
            if mods[i] {
                return Err(ChordError::RepeatedModifier(word.to_string()));
            }
            mods[i] = true;
            continue;
        }
        if !is_key(word) {
            return Err(ChordError::UnknownKey(word.to_string()));
        }
        if let Some(first) = &key {
            return Err(ChordError::TwoKeys(first.clone(), word.to_string()));
        }
        key = Some(word.to_string());
    }
    let key = key.ok_or(ChordError::NoKey)?;
    let mut out: Vec<&str> = MODIFIERS
        .iter()
        .zip(mods)
        .filter_map(|(m, on)| on.then_some(*m))
        .collect();
    out.push(&key);
    Ok(out.join("+"))
}

/// One `chord=target` pair of a shell's `shortcuts=`, as authored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutPair<'a> {
    /// The chord as written (not yet read).
    pub chord: &'a str,
    /// What it triggers: a `#seat` / route (`#surfy`, `/tasks`) or an action
    /// verb (`openSearch`, `closeTab`). Empty when the pair had no `=`.
    pub target: &'a str,
}

/// Split a `shortcuts=` value into its pairs (whitespace-separated
/// `chord=target`). Nothing is checked here; [`parse_shortcuts`] and the
/// lint read the pairs.
pub fn shortcut_pairs(raw: &str) -> Vec<ShortcutPair<'_>> {
    raw.split_whitespace()
        .map(|pair| match pair.split_once('=') {
            Some((chord, target)) => ShortcutPair { chord, target },
            None => ShortcutPair { chord: pair, target: "" },
        })
        .collect()
}

/// The readable pairs of a `shortcuts=` value, chords normalised. A pair
/// with a bad chord or no target is left out (lint L050 names it).
pub fn parse_shortcuts(raw: &str) -> Vec<(String, String)> {
    shortcut_pairs(raw)
        .into_iter()
        .filter(|p| !p.target.is_empty())
        .filter_map(|p| parse_chord(p.chord).ok().map(|c| (c, p.target.to_string())))
        .collect()
}

/// A `focus=` value, normalised (`"3"`, `"none"`), or `None` when it is not
/// a whole number 1–9999 or `none`.
pub fn focus_value(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some("none".to_string());
    }
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match raw.parse::<u32>() {
        Ok(n) if (1..=9999).contains(&n) => Some(n.to_string()),
        _ => None,
    }
}

fn closed(raw: &str, vocab: &'static [&'static str]) -> Option<&'static str> {
    let raw = raw.trim();
    vocab.iter().copied().find(|v| v.eq_ignore_ascii_case(raw))
}

/// A `reveal=` value, or `None` when it is not one of [`REVEALS`].
pub fn reveal_value(raw: &str) -> Option<&'static str> {
    closed(raw, REVEALS)
}

/// A `native=` value, or `None` when it is not on [`NATIVE_SEATS`].
pub fn native_value(raw: &str) -> Option<&'static str> {
    closed(raw, NATIVE_SEATS)
}

/// A `pane=` value, or `None` when it is not one of [`PANES`].
pub fn pane_value(raw: &str) -> Option<&'static str> {
    closed(raw, PANES)
}

/// A `panes=` value, or `None` when it is not one of [`PANE_COUNTS`].
pub fn panes_value(raw: &str) -> Option<&'static str> {
    closed(raw, PANE_COUNTS)
}

/// A `panes-at=` value, or `None` when it is not one of [`PANES_AT`].
pub fn panes_at_value(raw: &str) -> Option<&'static str> {
    closed(raw, PANES_AT)
}

/// `true` when `raw` is a well-formed surface id: a lower-case kind, a dot,
/// then one or more dot-separated names of letters, digits, `-` and `_`
/// (`sheet.invite`, `panel.tasks`, `action.openCreate`,
/// `control.surfy-tier.sublime`) — the shape of the parity ledger's rows.
pub fn is_surface_id(raw: &str) -> bool {
    let mut parts = raw.split('.');
    let Some(kind) = parts.next() else { return false };
    let kind_ok = kind.starts_with(|c: char| c.is_ascii_lowercase())
        && kind.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !kind_ok {
        return false;
    }
    let mut names = 0;
    for name in parts {
        names += 1;
        let ok = name.starts_with(|c: char| c.is_ascii_alphanumeric())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok {
            return false;
        }
    }
    names > 0
}

/// The interaction facts of one block, read from its attributes. Every value
/// is already checked and normalised: a value that does not read is left
/// out here (the lint names it), so a renderer emits only what a player can
/// act on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interaction {
    /// `surface=` — the ledger id.
    pub surface: Option<String>,
    /// `key=` — the normalised chord.
    pub key: Option<String>,
    /// `focus=` — a number 1–9999 or `none`.
    pub focus: Option<String>,
    /// `reveal=` — `always` or `hover`.
    pub reveal: Option<String>,
    /// `native=` — one of [`NATIVE_SEATS`].
    pub native: Option<String>,
    /// `pane=` — `start` or `end`.
    pub pane: Option<String>,
    /// `panes=` (`::app-shell` only) — `one` or `two`.
    pub panes: Option<String>,
    /// `panes-at=` (`::app-shell` only) — `tablet` or `desktop`.
    pub panes_at: Option<String>,
    /// `shortcuts=` (`::app-shell` only) — `(chord, target)` pairs.
    pub shortcuts: Vec<(String, String)>,
}

fn text(attrs: &Attrs, name: &str) -> Option<String> {
    let raw = match attrs.get(name)? {
        AttrValue::String(s) => s.clone(),
        AttrValue::Number(n) => n.to_string(),
        AttrValue::Bool(b) => b.to_string(),
        AttrValue::Null => return None,
    };
    let raw = raw.trim().to_string();
    if raw.is_empty() { None } else { Some(raw) }
}

impl Interaction {
    /// Read a directive's attributes. `name` is the directive: the
    /// shell-only attributes are read on `app-shell` alone, and `key=` is
    /// not read on a directive that spends `key=` on its own meaning
    /// (`::cite[key=smith2020]`).
    pub fn from_attrs(name: &str, attrs: &Attrs) -> Self {
        let mut out = Interaction {
            surface: text(attrs, "surface").filter(|s| is_surface_id(s)),
            focus: text(attrs, "focus").and_then(|s| focus_value(&s)),
            reveal: text(attrs, "reveal").and_then(|s| reveal_value(&s)).map(str::to_string),
            native: text(attrs, "native").and_then(|s| native_value(&s)).map(str::to_string),
            pane: text(attrs, "pane").and_then(|s| pane_value(&s)).map(str::to_string),
            ..Interaction::default()
        };
        if !key_is_typed(name) {
            out.key = text(attrs, "key").and_then(|s| parse_chord(&s).ok());
        }
        if name == "app-shell" {
            out.panes = text(attrs, "panes").and_then(|s| panes_value(&s)).map(str::to_string);
            if out.panes.as_deref() == Some("two") {
                out.panes_at = Some(
                    text(attrs, "panes-at")
                        .and_then(|s| panes_at_value(&s))
                        .unwrap_or("tablet")
                        .to_string(),
                );
            }
            out.shortcuts = text(attrs, "shortcuts").map(|s| parse_shortcuts(&s)).unwrap_or_default();
        }
        out
    }

    /// `true` when nothing was authored (nothing to emit).
    pub fn is_empty(&self) -> bool {
        self == &Interaction::default()
    }

    /// The `data-*` attributes this block carries in HTML and the DOM twin,
    /// in emission order. `shortcuts` serialises back to `chord=target`
    /// pairs separated by one space.
    pub fn data_attrs(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let mut push = |name: &'static str, v: &Option<String>| {
            if let Some(v) = v {
                out.push((name, v.clone()));
            }
        };
        push("data-surface", &self.surface);
        push("data-key", &self.key);
        push("data-focus", &self.focus);
        push("data-reveal", &self.reveal);
        push("data-native", &self.native);
        push("data-pane", &self.pane);
        push("data-panes", &self.panes);
        push("data-panes-at", &self.panes_at);
        if !self.shortcuts.is_empty() {
            out.push(("data-shortcuts", join_shortcuts(&self.shortcuts)));
        }
        out
    }
}

/// `chord=target` pairs joined by one space — the normalised `shortcuts=`.
pub fn join_shortcuts(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(c, t)| format!("{c}={t}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `data-*` attributes of a toolbar `- button[…]` item, from its authored
/// `key=` / `focus=` / `reveal=` (each emitted only when it reads).
pub fn item_data_attrs(
    key: Option<&str>,
    focus: Option<&str>,
    reveal: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(k) = key.and_then(|k| parse_chord(k).ok()) {
        out.push(("data-key", k));
    }
    if let Some(f) = focus.and_then(focus_value) {
        out.push(("data-focus", f));
    }
    if let Some(r) = reveal.and_then(reveal_value) {
        out.push(("data-reveal", r.to_string()));
    }
    out
}

/// Directives that spend `key=` on their own meaning: the registry rows
/// that list `key` among their attributes (`cite`), plus `cite`'s alias
/// `reference-def`. On these `key=` is never read as a chord.
pub fn key_is_typed(name: &str) -> bool {
    name == "reference-def" || crate::lint::blocks_with_typed_key().contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_normalise() {
        assert_eq!(parse_chord("mod+k").unwrap(), "mod+k");
        assert_eq!(parse_chord("Shift+Mod+K").unwrap(), "mod+shift+k");
        assert_eq!(parse_chord("mod+1").unwrap(), "mod+1");
        assert_eq!(parse_chord("alt+ctrl+f12").unwrap(), "ctrl+alt+f12");
        assert_eq!(parse_chord("escape").unwrap(), "escape");
        assert_eq!(parse_chord("mod+comma").unwrap(), "mod+comma");
    }

    #[test]
    fn chords_refuse() {
        assert_eq!(parse_chord(""), Err(ChordError::Empty));
        assert!(matches!(parse_chord("cmd+k"), Err(ChordError::PlatformWord { use_instead: "mod", .. })));
        assert!(matches!(parse_chord("option+k"), Err(ChordError::PlatformWord { use_instead: "alt", .. })));
        assert_eq!(parse_chord("mod+mod+k"), Err(ChordError::RepeatedModifier("mod".into())));
        assert_eq!(parse_chord("mod+shift"), Err(ChordError::NoKey));
        assert_eq!(parse_chord("mod+k+j"), Err(ChordError::TwoKeys("k".into(), "j".into())));
        assert_eq!(parse_chord("mod+kk"), Err(ChordError::UnknownKey("kk".into())));
        assert!(parse_chord("mod+").is_err());
        assert!(parse_chord("mod+f13").is_err());
        assert!(parse_chord("mod+f0").is_err());
        assert!(parse_chord("mod+f01").is_err());
    }

    #[test]
    fn shortcuts_split_and_read() {
        let pairs = parse_shortcuts("mod+1=#surfy  Mod+K=openSearch cmd+w=closeTab mod+2 mod+3=");
        assert_eq!(
            pairs,
            vec![("mod+1".to_string(), "#surfy".to_string()), ("mod+k".to_string(), "openSearch".to_string())]
        );
    }

    #[test]
    fn closed_values() {
        assert_eq!(focus_value("3").as_deref(), Some("3"));
        assert_eq!(focus_value("None").as_deref(), Some("none"));
        assert_eq!(focus_value("0"), None);
        assert_eq!(focus_value("-1"), None);
        assert_eq!(focus_value("10000"), None);
        assert_eq!(reveal_value("hover"), Some("hover"));
        assert_eq!(reveal_value("focus"), None);
        assert_eq!(native_value("terminal"), Some("terminal"));
        assert_eq!(native_value("printer"), None);
        assert_eq!(pane_value("end"), Some("end"));
        assert_eq!(panes_value("two"), Some("two"));
        assert_eq!(panes_at_value("mobile"), None);
    }

    #[test]
    fn surface_ids() {
        for ok in ["sheet.invite", "panel.tasks", "action.openCreate", "action.open_doc", "control.surfy-tier.sublime"] {
            assert!(is_surface_id(ok), "{ok}");
        }
        for bad in ["", "invite", "Sheet.invite", "sheet.", ".invite", "sheet..invite", "sheet.in vite", "sheet.-x"] {
            assert!(!is_surface_id(bad), "{bad}");
        }
    }

    #[test]
    fn cite_keeps_its_key() {
        assert!(key_is_typed("cite"));
        assert!(key_is_typed("reference-def"));
        assert!(!key_is_typed("row"));
    }
}
