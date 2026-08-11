//! Built-in SVG icon set for SurfDoc.
//!
//! A curated subset of icons for use in `::cta` and `::nav` blocks via `icon=` attribute.
//! Each icon is a minimal inline SVG (24x24 viewbox, stroke-based, currentColor).

/// Look up a built-in icon by name. Returns an inline SVG string or `None`.
///
/// Resolution order (0.13.3):
/// 1. the curated built-in constants below (stable — existing renders keep
///    their exact glyphs);
/// 2. the vendored surf-icons set (`icons_vendored.rs`, 127 glyphs from
///    CloudSurf's own MIT icon library);
/// 3. the design-vocabulary alias table (`doc` → `docs`, `knowledge` →
///    `messages`, …), resolved back through 1 then 2.
///
/// Unknown names return `None`; row/toolbar renderers keep their circle
/// fallback. Icons are a render concern only — no block schema involvement.
pub fn get_icon(name: &str) -> Option<&'static str> {
    if let Some(svg) = builtin_icon(name) {
        return Some(svg);
    }
    if let Some(svg) = crate::icons_vendored::vendored_icon(name) {
        return Some(svg);
    }
    let alias = design_alias(name)?;
    builtin_icon(alias).or_else(|| crate::icons_vendored::vendored_icon(alias))
}

/// Design-vocabulary aliases — names the Surfspace shell docs use that map
/// onto a differently-named glyph. Kept as data so the vocabulary is
/// auditable in one place.
fn design_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "doc" => "docs",
        "task" => "tasks",
        // Every authored `icon=knowledge` site is messages-semantic (nav rows,
        // roster rows, tab bar) — the alias exists solely to glyph the
        // messages nav row, so it resolves to the filled messages trace.
        // `book-open` stays reachable under its own name.
        "knowledge" => "messages",
        "posts" => "newspaper",
        "device" => "devices",
        "sort" => "list",
        "move" => "arrow-right",
        "feature" => "star",
        "double-check" => "check-circle",
        "people" | "members" => "users",
        "import" => "upload",
        "wiki" => "book",
        "signin" => "login",
        "ai" => "sparkle",
        _ => return None,
    })
}

/// The curated built-in constants (pre-0.13.3 set plus the sparkle/login
/// glyphs folded in from the private row-icon table).
fn builtin_icon(name: &str) -> Option<&'static str> {
    match name {
        "download" => Some(ICON_DOWNLOAD),
        "github" => Some(ICON_GITHUB),
        "external-link" => Some(ICON_EXTERNAL_LINK),
        "arrow-right" => Some(ICON_ARROW_RIGHT),
        "chevron-right" => Some(ICON_CHEVRON_RIGHT),
        "mail" => Some(ICON_MAIL),
        "star" => Some(ICON_STAR),
        "heart" => Some(ICON_HEART),
        "play" => Some(ICON_PLAY),
        "book" => Some(ICON_BOOK),
        "code" => Some(ICON_CODE),
        "globe" => Some(ICON_GLOBE),
        "check" => Some(ICON_CHECK),
        "info" => Some(ICON_INFO),
        "menu" => Some(ICON_MENU),
        "search" => Some(ICON_SEARCH),
        "settings" => Some(ICON_SETTINGS),
        "user" => Some(ICON_USER),
        "home" => Some(ICON_HOME),
        "file" => Some(ICON_FILE),
        "file-text" => Some(ICON_FILE_TEXT),
        "clock" => Some(ICON_CLOCK),
        "edit" | "pencil" => Some(ICON_EDIT),
        "shield" => Some(ICON_SHIELD),
        "zap" => Some(ICON_ZAP),
        "lock" => Some(ICON_LOCK),
        "phone" => Some(ICON_PHONE),
        "map-pin" => Some(ICON_MAP_PIN),
        "calendar" => Some(ICON_CALENDAR),
        "users" => Some(ICON_USERS),
        "truck" => Some(ICON_TRUCK),
        "message-circle" => Some(ICON_MESSAGE_CIRCLE),
        "image" => Some(ICON_IMAGE),
        "briefcase" => Some(ICON_BRIEFCASE),
        "award" => Some(ICON_AWARD),
        "layers" => Some(ICON_LAYERS),
        "package" => Some(ICON_PACKAGE),
        "trending-up" => Some(ICON_TRENDING_UP),
        "coffee" => Some(ICON_COFFEE),
        "scissors" => Some(ICON_SCISSORS),
        "wrench" => Some(ICON_WRENCH),
        "target" => Some(ICON_TARGET),
        "flask" | "beaker" => Some(ICON_FLASK),
        "newspaper" | "news" => Some(ICON_NEWSPAPER),
        "sparkle" => Some(ICON_SPARKLE),
        "login" => Some(ICON_LOGIN),
        "bug" => Some(ICON_BUG),
        _ => None,
    }
}

/// Return the list of all available icon names: built-in constants,
/// design-vocabulary aliases, and the vendored surf-icons set, deduped.
pub fn available_icons() -> &'static [&'static str] {
    &[
        "activity", "ai", "alert-circle", "apps", "archive", "arrow-down", "arrow-left",
        "arrow-right", "arrow-up", "award", "bar-chart", "beaker", "book", "book-open",
        "bookmark", "box", "briefcase", "bug", "calendar", "camera", "check", "check-circle",
        "chevron-down", "chevron-left", "chevron-right", "chevron-up", "circle", "clipboard",
        "clock", "cloud", "code", "code-repos", "coffee", "compass", "copy", "create",
        "credit-card", "database", "deploy", "device", "devices", "doc", "docs", "dollar-sign",
        "domains", "double-check", "download", "edit", "external-link", "feature", "file",
        "file-text", "files", "filter", "flag", "flask", "folder", "gift", "github", "globe",
        "heart", "home", "image", "images", "import", "inbox", "info", "knowledge", "layers",
        "layout", "link", "list", "lock", "login", "mail", "map", "map-pin", "members", "menu",
        "message-circle", "message-square", "messages", "mic", "minus", "monitor", "moon",
        "more", "move", "music", "new-chat", "news", "newspaper", "notebook", "notifications",
        "package", "pencil", "people", "phone", "pie-chart", "play", "plus", "plus-circle",
        "posts", "refresh-cw", "ruler", "scissors", "search", "send", "settings", "share",
        "shield", "shopping-bag", "shopping-cart", "signin", "smartphone", "sort", "sparkle",
        "star", "sun", "surf-emblem", "surfy", "surfy-fin", "tag", "target", "task", "tasks",
        "thumbs-down", "thumbs-up", "tier-free", "tier-premium", "tier-pro", "tier-ultra",
        "trash", "trending-up", "trophy", "truck", "upload", "user", "users", "video",
        "wallet", "waves", "wavesite", "wiki", "wind", "workspace", "wrench", "x", "zap",
    ]
}

// All icons: 24x24, stroke-based, currentColor, no fill, 1.5 stroke-width.
// Based on Lucide icon paths (MIT license).

const ICON_DOWNLOAD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>"#;

const ICON_GITHUB: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z"/></svg>"#;

const ICON_EXTERNAL_LINK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>"#;

const ICON_ARROW_RIGHT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>"#;

const ICON_CHEVRON_RIGHT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>"#;

const ICON_MAIL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/></svg>"#;

const ICON_STAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>"#;

const ICON_HEART: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/></svg>"#;

const ICON_PLAY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>"#;

const ICON_BOOK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/></svg>"#;

const ICON_CODE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>"#;

const ICON_GLOBE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/></svg>"#;

const ICON_CHECK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>"#;

const ICON_INFO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"#;

const ICON_MENU: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="4" y1="12" x2="20" y2="12"/><line x1="4" y1="6" x2="20" y2="6"/><line x1="4" y1="18" x2="20" y2="18"/></svg>"#;

const ICON_SEARCH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;

const ICON_SETTINGS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>"#;

const ICON_USER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>"#;

const ICON_HOME: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>"#;

const ICON_FILE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>"#;

const ICON_FILE_TEXT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>"#;

const ICON_FLASK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 3h6"/><path d="M10 3v6.5L5 18a2 2 0 0 0 1.7 3h10.6A2 2 0 0 0 19 18l-5-8.5V3"/><path d="M7 14h10"/></svg>"#;

const ICON_NEWSPAPER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"/><line x1="18" y1="14" x2="10" y2="14"/><line x1="15" y1="18" x2="10" y2="18"/><rect x="10" y="6" width="8" height="4"/></svg>"#;

const ICON_CLOCK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>"#;

const ICON_EDIT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/></svg>"#;

const ICON_SHIELD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10"/></svg>"#;

const ICON_ZAP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>"#;

const ICON_LOCK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>"#;

const ICON_PHONE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/></svg>"#;

const ICON_MAP_PIN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0Z"/><circle cx="12" cy="10" r="3"/></svg>"#;

const ICON_CALENDAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2v4"/><path d="M16 2v4"/><rect width="18" height="18" x="3" y="4" rx="2"/><path d="M3 10h18"/></svg>"#;

const ICON_USERS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#;

const ICON_TRUCK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 18V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v11a1 1 0 0 0 1 1h2"/><path d="M15 18h2a1 1 0 0 0 1-1v-3.65a1 1 0 0 0-.22-.624l-3.48-4.35A1 1 0 0 0 13.52 8H12"/><circle cx="17" cy="18" r="2"/><circle cx="7" cy="18" r="2"/></svg>"#;

const ICON_MESSAGE_CIRCLE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/></svg>"#;

const ICON_IMAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>"#;

const ICON_BRIEFCASE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 20V4a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"/><rect width="20" height="14" x="2" y="6" rx="2"/></svg>"#;

const ICON_AWARD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="6"/><path d="M15.477 12.89 17 22l-5-3-5 3 1.523-9.11"/></svg>"#;

const ICON_LAYERS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/></svg>"#;

const ICON_PACKAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.5 4.27 9 5.15"/><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>"#;

const ICON_TRENDING_UP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/><polyline points="16 7 22 7 22 13"/></svg>"#;

const ICON_COFFEE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 2v2"/><path d="M14 2v2"/><path d="M16 8a1 1 0 0 1 1 1v8a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V9a1 1 0 0 1 1-1h14a4 4 0 1 1 0 8h-1"/><path d="M6 2v2"/></svg>"#;

const ICON_SCISSORS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="3"/><path d="M8.12 8.12 12 12"/><path d="M20 4 8.12 15.88"/><circle cx="6" cy="18" r="3"/><path d="M14.8 14.8 20 20"/></svg>"#;

const ICON_WRENCH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76Z"/></svg>"#;

const ICON_TARGET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/></svg>"#;

// Folded in from the private row-icon table (0.13.3): sparkle (Surfy/AI) and
// login have no surf-icons counterpart, so they stay curated constants.

const ICON_SPARKLE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/></svg>"#;

const ICON_LOGIN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><polyline points="10 17 15 12 10 7"/><line x1="15" y1="12" x2="3" y2="12"/></svg>"#;

// Bug-type task rows — glyph traced from preview/mockup.html (design ground
// truth, 2026-08-10 ruling): rounded-body beetle with antennae + legs.
const ICON_BUG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="8" y="6" width="8" height="14" rx="4"/><path d="M9 6a3 3 0 0 1 6 0"/><path d="M12 11v9"/><path d="M8 12H5M8 17H5M16 12h3M16 17h3"/></svg>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_icons_resolve() {
        for name in available_icons() {
            assert!(get_icon(name).is_some(), "Icon '{}' should resolve", name);
        }
    }

    #[test]
    fn unknown_icon_returns_none() {
        assert!(get_icon("nonexistent").is_none());
        // bug resolves as of the consolidation round: preview/mockup.html
        // (design ground truth) draws a real beetle glyph on bug-type task
        // rows, so the earlier "circle fallback by design" call was wrong
        // against the mockup.
        assert!(get_icon("bug").is_some());
    }

    #[test]
    fn design_vocabulary_names_resolve() {
        // Every icon name the Surfspace shell strategy docs use must resolve
        // through the registry (measured vocabulary, 2026-08-10 round).
        for name in [
            "doc", "task", "apps", "knowledge", "posts", "folder", "device",
            "settings", "notifications", "search", "surfy-fin", "login",
            "file", "sparkle", "chevron-left", "plus", "share", "filter",
            "edit", "download", "copy", "upload", "trash", "sort", "people",
            "package", "move", "members", "lock", "import", "feature",
            "double-check", "clock", "archive", "bug",
        ] {
            assert!(get_icon(name).is_some(), "design name '{}' must resolve", name);
        }
        // knowledge is a messages-semantic alias (every authored site is a
        // messages nav/roster row): it must resolve to the same glyph as
        // `messages`, while `book-open` keeps its own distinct stroke.
        assert_eq!(
            get_icon("knowledge"),
            get_icon("messages"),
            "knowledge must alias the messages trace"
        );
        let book_open = get_icon("book-open").expect("book-open must still resolve");
        assert_ne!(
            Some(book_open),
            get_icon("messages"),
            "book-open must keep its own distinct stroke"
        );
    }

    #[test]
    fn vendored_icons_all_resolve_and_are_valid() {
        for (name, svg) in crate::icons_vendored::VENDORED_ICONS {
            // Builtin constants shadow same-named vendored glyphs by design
            // (stability), so assert resolution, not identity.
            assert!(get_icon(name).is_some(), "vendored '{}' must resolve", name);
            assert!(svg.starts_with("<svg"), "vendored '{}' must start with <svg", name);
            assert!(svg.ends_with("</svg>"), "vendored '{}' must end with </svg>", name);
            assert!(svg.contains("currentColor"), "vendored '{}' must use currentColor", name);
            assert!(
                svg.contains("width=\"16\" height=\"16\""),
                "vendored '{}' must carry the intrinsic 16px size",
                name
            );
        }
    }

    #[test]
    fn vendored_table_is_sorted_for_binary_search() {
        let names: Vec<&str> = crate::icons_vendored::VENDORED_ICONS.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "VENDORED_ICONS must stay sorted by name");
    }

    #[test]
    fn builtin_names_keep_their_curated_glyphs() {
        // Resolution order pins stability: names that predate the vendored
        // set keep their exact constants, so existing renders do not drift.
        assert_eq!(get_icon("download"), Some(ICON_DOWNLOAD));
        assert_eq!(get_icon("search"), Some(ICON_SEARCH));
        assert_eq!(get_icon("settings"), Some(ICON_SETTINGS));
        assert_eq!(get_icon("newspaper"), Some(ICON_NEWSPAPER));
    }

    #[test]
    fn icons_are_valid_svg() {
        for name in available_icons() {
            let svg = get_icon(name).unwrap();
            assert!(svg.starts_with("<svg"), "Icon '{}' must start with <svg", name);
            assert!(svg.ends_with("</svg>"), "Icon '{}' must end with </svg>", name);
        }
    }

    #[test]
    fn icons_use_current_color() {
        for name in available_icons() {
            let svg = get_icon(name).unwrap();
            assert!(svg.contains("currentColor"), "Icon '{}' must use currentColor", name);
        }
    }
}
