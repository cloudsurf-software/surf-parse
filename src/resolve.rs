//! Shared resolution step — style packs, fonts, and contrast math.
//!
//! This module is the single owner of token semantics for BOTH render
//! targets: `render_html` (websites) consumes these functions for its CSS
//! emission, and `render_native` projects the same resolved values across
//! the FFI as a `NativeTheme`. The 0025 style-pack property — "preview
//! cannot drift from the pack" — holds only because resolution happens
//! exactly once, here, in Rust. Never reimplement pack→value mapping in
//! Swift/Kotlin/JS.
//!
//! The `--ws-*` token contract (style-pack rendering annex v0):
//! packs re-set ONLY these tokens; **packs change visuals, never layout**.
//! Defaults are the "Surf Simple" pack and must stay in lockstep with
//! `assets/surfdoc.css` `:root` (drift-tested in `render_html`).
//!
//! The pack registry here mirrors the production injection tables in
//! `repos/surf` `frontend/src/sites/container.rs` (`STYLE_PACKS`). The
//! container remains the web injection point; this module is the canonical
//! semantic owner for native targets and any future consolidation.

// ═══════════════════════════════════════════════════════════════════════
// Fonts
// ═══════════════════════════════════════════════════════════════════════

/// A resolved font preset: CSS font stack + optional Google Fonts import URL.
pub(crate) struct FontPreset {
    pub(crate) stack: &'static str,
    pub(crate) import: Option<&'static str>,
}

/// Resolve a font preset name to a CSS font stack and optional import.
pub(crate) fn resolve_font_preset(name: &str) -> Option<FontPreset> {
    match name.trim().to_lowercase().as_str() {
        "system" | "sans" => Some(FontPreset {
            stack: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Oxygen, sans-serif",
            import: None,
        }),
        "serif" | "editorial" => Some(FontPreset {
            stack: "Georgia, \"Palatino Linotype\", \"Book Antiqua\", Palatino, serif",
            import: None,
        }),
        "mono" | "monospace" | "technical" => Some(FontPreset {
            stack: "\"SF Mono\", \"Fira Code\", \"Cascadia Code\", Menlo, Consolas, monospace",
            import: None,
        }),
        "inter" => Some(FontPreset {
            stack: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"),
        }),
        "montserrat" => Some(FontPreset {
            stack: "'Montserrat', sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Montserrat:wght@400;600;700;800&display=swap"),
        }),
        "jetbrains-mono" | "jetbrains" => Some(FontPreset {
            stack: "'JetBrains Mono', monospace",
            import: Some("https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&display=swap"),
        }),
        "playfair" | "playfair-display" => Some(FontPreset {
            stack: "'Playfair Display', Georgia, serif",
            import: Some("https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;500;600;700&display=swap"),
        }),
        // Surf's own display face. No import URL on purpose: the font files
        // are self-hosted by consumers (@font-face in the host page/app
        // bundle), so the preset only supplies the stack — with a graceful
        // system-sans fallback for surfaces that don't ship the files.
        "surf-display" => Some(FontPreset {
            stack: "'Surf Display', -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, sans-serif",
            import: None,
        }),
        "lato" => Some(FontPreset {
            stack: "'Lato', -apple-system, BlinkMacSystemFont, sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Lato:wght@300;400;700&display=swap"),
        }),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Color math (WCAG 2.x)
// ═══════════════════════════════════════════════════════════════════════

/// Parse a hex color (#RGB, #RRGGBB) into RGB components. `None` when the
/// string isn't a parseable hex color.
pub(crate) fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        3 => Some((
            u8::from_str_radix(&hex[0..1], 16).ok()? * 17,
            u8::from_str_radix(&hex[1..2], 16).ok()? * 17,
            u8::from_str_radix(&hex[2..3], 16).ok()? * 17,
        )),
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        _ => None,
    }
}

/// WCAG 2.x relative luminance of an sRGB color.
pub(crate) fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn linearize(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG 2.x contrast ratio between two hex colors (1.0..=21.0).
/// Unparseable input yields 0.0 (so "is this AA?" checks fail loudly).
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (Some((ar, ag, ab)), Some((br, bg, bb))) = (parse_hex_rgb(a), parse_hex_rgb(b)) else {
        return 0.0;
    };
    let la = relative_luminance(ar, ag, ab);
    let lb = relative_luminance(br, bg, bb);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Parse a hex color (#RGB, #RRGGBB) and return the WCAG-compliant text color.
/// Returns "#fff" for dark accents, "#1a1a2e" for light accents.
pub(crate) fn accent_text_color(hex: &str) -> &'static str {
    let Some((r, g, b)) = parse_hex_rgb(hex) else {
        return "#fff"; // Can't parse — default to white
    };
    let lum = relative_luminance(r, g, b);
    // Threshold 0.25: ensures minimum ~3.5:1 contrast ratio (WCAG AA for large
    // text / UI components). Catches greens, yellows, ambers while keeping
    // standard blues (#3b82f6) and reds (#ef4444) with white text.
    if lum > 0.25 { "#1a1a2e" } else { "#fff" }
}

/// The strictest background an accent faces as TEXT per theme: the light
/// theme's `--surface-alt` (#eef1f7) and the dark theme's `--surface-alt`
/// (#161616). AA on these implies AA on the lighter/darker page + sheet
/// surfaces of the same theme.
const INK_TARGET_LIGHT_BG: &str = "#eef1f7";
const INK_TARGET_DARK_BG: &str = "#161616";

/// Derive an accent **ink** — the accent adjusted until it reads as normal
/// TEXT at WCAG AA (≥ 4.5:1) against the theme's surfaces (BR-SITE-A11Y).
///
/// `accent_text_color` solves text ON an accent background; this solves the
/// accent AS text (active nav link, prose links, secondary CTA labels). For
/// the light theme the accent is darkened in-hue (×0.96 per step toward
/// black); for the dark theme it is lightened (5% toward white per step).
/// Accents that already pass are returned unchanged. Unparseable input falls
/// back to the theme's safe ink.
pub fn accent_ink_color(hex: &str, dark: bool) -> String {
    let Some((mut r, mut g, mut b)) = parse_hex_rgb(hex) else {
        return if dark { "#ffffff".into() } else { "#1a1a2e".into() };
    };
    let target = if dark { INK_TARGET_DARK_BG } else { INK_TARGET_LIGHT_BG };
    let (tr, tg, tb) = parse_hex_rgb(target).expect("ink targets are valid hex");
    let bg_lum = relative_luminance(tr, tg, tb);
    for _ in 0..200 {
        let lum = relative_luminance(r, g, b);
        let (hi, lo) = if lum >= bg_lum { (lum, bg_lum) } else { (bg_lum, lum) };
        if (hi + 0.05) / (lo + 0.05) >= 4.5 {
            break;
        }
        if dark {
            r = (r as f64 + (255.0 - r as f64) * 0.05).round().min(255.0) as u8;
            g = (g as f64 + (255.0 - g as f64) * 0.05).round().min(255.0) as u8;
            b = (b as f64 + (255.0 - b as f64) * 0.05).round().min(255.0) as u8;
        } else {
            r = (r as f64 * 0.96).round() as u8;
            g = (g as f64 * 0.96).round() as u8;
            b = (b as f64 * 0.96).round() as u8;
        }
    }
    format!("#{r:02x}{g:02x}{b:02x}")
}

// ═══════════════════════════════════════════════════════════════════════
// Style packs — the --ws-* token contract (annex v0)
// ═══════════════════════════════════════════════════════════════════════

/// Platform default accent (`--accent` in `assets/surfdoc.css` `:root`).
pub const DEFAULT_ACCENT: &str = "#2563eb";

/// Default style-pack key. Mirrors `DEFAULT_STYLE_PACK` in the surf
/// container (0025 DEFAULT).
pub const DEFAULT_STYLE_PACK: &str = "surf";

/// The resolved `--ws-*` token values for one style pack. Values are the
/// exact CSS strings the web renderer emits; native targets parse the px
/// fields into numeric form via `NativeTheme`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsTokens {
    pub radius_card: &'static str,
    pub radius_btn: &'static str,
    pub radius_chip: &'static str,
    pub radius_img: &'static str,
    pub border_w: &'static str,
    pub border_style: &'static str,
    pub shadow: &'static str,
    pub shadow_hover: &'static str,
    pub bg_texture: &'static str,
    pub hero_bg: &'static str,

    // ── SS-1 additive component tokens (0.9.3) ─────────────────────────
    // Var names are the de-facto contract set by the cloudsurf-website
    // app.css `:root` (field name = snake_case of the `--ws-*` var; pinned
    // by `ss1_additive_token_var_name_contract` below). Defaults are
    // IDENTITY values: each equals the surfdoc.css `:root` default or the
    // per-element fallback it mirrors, so a pack that does not re-set a
    // token renders pixel-identical (drift-tested below).
    /// Hero action-button corner (`--ws-hero-btn-radius`).
    pub hero_btn_radius: &'static str,
    /// `::banner` action-button corner (`--ws-banner-btn-radius`).
    pub banner_btn_radius: &'static str,
    /// Standalone CTA button corner (`--ws-cta-radius`).
    pub cta_radius: &'static str,
    /// `::form` submit-button corner (`--ws-form-submit-radius`).
    pub form_submit_radius: &'static str,
    /// App-control corner (`--ws-control-radius`). Identity value is
    /// `initial` — the CSS guaranteed-invalid value — because consumers
    /// carry DIVERGENT per-element fallbacks (forms `var(--radius-sm)`,
    /// booking/store controls 8px, store badges/qty 6px); `initial` keeps
    /// every `var(--ws-control-radius, <fallback>)` on its own fallback,
    /// so injecting the identity pack stays a visual no-op. surfdoc.css
    /// intentionally declares no `:root` value for it (see its Phase-2
    /// comment block).
    pub control_radius: &'static str,
    /// Feature-card corner (`--ws-feature-card-radius`).
    pub feature_card_radius: &'static str,
    /// Feature-card padding (`--ws-feature-card-pad`).
    pub feature_card_pad: &'static str,
    /// Feature-card hover lift (`--ws-feature-card-hover-transform`).
    pub feature_card_hover_transform: &'static str,
    /// Feature-card surface (`--ws-feature-card-bg`).
    pub feature_card_bg: &'static str,
    /// `::product-grid` tile-surface fill (`--ws-tile-surface-bg`).
    pub tile_surface_bg: &'static str,
    /// `::post-grid` card surface (`--ws-post-card-bg`).
    pub post_card_bg: &'static str,
    /// `::post-grid` card corner (`--ws-post-card-radius`).
    pub post_card_radius: &'static str,
    /// `::product-grid` row-card surface (`--ws-pg-card-bg`).
    pub pg_card_bg: &'static str,
    /// `::product-grid` row-card corner (`--ws-pg-card-radius`).
    pub pg_card_radius: &'static str,
    /// `::product-grid` tile corner (`--ws-pg-tile-radius`; square by spec).
    pub pg_tile_radius: &'static str,
    /// `::details` disclosure surface (`--ws-details-bg`).
    pub details_bg: &'static str,
    /// `::details` corner (`--ws-details-radius`).
    pub details_radius: &'static str,
    /// `::form` input fill (`--ws-form-input-bg`).
    pub form_input_bg: &'static str,
    /// Doc-page sheet surface (`--ws-doc-page-bg`).
    pub doc_page_bg: &'static str,
    /// Doc-page sheet corner (`--ws-doc-page-radius`).
    pub doc_page_radius: &'static str,
    /// Shell drawer link font size (`--ws-drawer-link-size`).
    pub drawer_link_size: &'static str,
    /// Shell drawer link font weight (`--ws-drawer-link-weight`).
    pub drawer_link_weight: &'static str,
}

/// Surf Simple — the zero-pack default. MUST stay byte-equal to the
/// `:root` declarations in `assets/surfdoc.css` (drift-tested below and in
/// `render_html`); injecting nothing on web yields exactly these values.
pub const SURF_SIMPLE_TOKENS: WsTokens = WsTokens {
    radius_card: "16px",
    radius_btn: "10px",
    radius_chip: "999px",
    radius_img: "10px",
    border_w: "1px",
    border_style: "solid",
    shadow: "0 1px 2px rgba(15, 23, 42, 0.04)",
    shadow_hover: "0 10px 30px rgba(15, 23, 42, 0.10)",
    bg_texture: "none",
    hero_bg: "radial-gradient(120% 80% at 50% 0%, rgba(255, 255, 255, 0.12), transparent 60%), linear-gradient(160deg, var(--accent), color-mix(in oklab, var(--accent) 68%, #0b1023))",
    // SS-1 identity defaults — each equals the surfdoc.css `:root` default
    // or the per-element fallback it mirrors (var() chains resolve against
    // whatever the pack sets for the base tokens, so they stay correct for
    // every pack that leaves them untouched).
    hero_btn_radius: "var(--ws-radius-btn)",
    banner_btn_radius: "var(--radius-sm)",
    cta_radius: "var(--ws-radius-btn)",
    form_submit_radius: "var(--ws-control-radius, var(--radius-sm))",
    control_radius: "initial", // guaranteed-invalid: per-element fallbacks stay live
    feature_card_radius: "var(--ws-radius-card)",
    feature_card_pad: "1.5rem",
    feature_card_hover_transform: "translateY(-2px)",
    feature_card_bg: "color-mix(in srgb, var(--surface) 50%, var(--surface-alt) 50%)",
    tile_surface_bg: "var(--surface)",
    post_card_bg: "var(--surface)",
    post_card_radius: "var(--ws-radius-card)",
    pg_card_bg: "color-mix(in srgb, var(--surface) 72%, transparent)",
    pg_card_radius: "var(--ws-radius-card-lg, 20px)",
    pg_tile_radius: "0",
    details_bg: "var(--surface-alt)",
    details_radius: "var(--radius-sm)",
    form_input_bg: "var(--surface)",
    doc_page_bg: "var(--surface)",
    doc_page_radius: "var(--ws-radius-card)",
    drawer_link_size: "0.9375rem",
    drawer_link_weight: "500",
};

/// Comic — the dramatic second voice that proves the contract: tight radii,
/// comic-thick borders, hard-offset panel shadows, flat hero ink, a light
/// diagonal hatch texture. Values mirror the `comic` pack in the surf
/// container's `STYLE_PACKS`.
pub const COMIC_TOKENS: WsTokens = WsTokens {
    radius_card: "4px",
    radius_btn: "6px",
    radius_chip: "6px",
    radius_img: "4px",
    border_w: "3px",
    border_style: "solid",
    shadow: "4px 4px 0 rgba(15, 23, 42, 0.85)",
    shadow_hover: "6px 6px 0 rgba(15, 23, 42, 0.85)",
    bg_texture: "repeating-linear-gradient(45deg, rgba(15, 23, 42, 0.025) 0 2px, transparent 2px 12px)",
    hero_bg: "var(--accent)",
    // SS-1 tokens: Comic does not re-set these in the production container
    // tables, so it carries the identity defaults — the var() chains follow
    // Comic's own base radii (e.g. hero buttons pick up the 6px btn corner).
    ..SURF_SIMPLE_TOKENS
};

/// Resolve a style-pack key to its token set. Unknown keys (DB drift,
/// forward-compat) fall back to Surf Simple — a stale pack value can never
/// break a render.
pub fn resolve_style_pack(key: &str) -> (&'static str, WsTokens) {
    match key.trim().to_lowercase().as_str() {
        "comic" => ("comic", COMIC_TOKENS),
        "surf" | "" => ("surf", SURF_SIMPLE_TOKENS),
        _ => ("surf", SURF_SIMPLE_TOKENS),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Resolved theme — what both renderers project
// ═══════════════════════════════════════════════════════════════════════

/// A document's fully resolved presentation: pack tokens with the pack
/// applied, accent + derived WCAG colors, and font stacks. Computed once;
/// `render_html` emits it as CSS variables, `render_native` ships it across
/// the FFI as `NativeTheme`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTheme {
    /// The resolved pack key ("surf", "comic", …).
    pub pack_id: String,
    /// Brand accent hex.
    pub accent: String,
    /// WCAG-compliant text color for content ON an accent background.
    pub on_accent: String,
    /// Accent adjusted to read as TEXT at AA on light surfaces.
    pub accent_ink_light: String,
    /// Accent adjusted to read as TEXT at AA on dark surfaces.
    pub accent_ink_dark: String,
    /// Display/heading font stack (CSS), if a preset resolved.
    pub font_display: Option<String>,
    /// Body font stack (CSS), if a preset resolved.
    pub font_body: Option<String>,
    /// The pack's `--ws-*` token values.
    pub tokens: WsTokens,
}

/// Resolve a theme from site-level inputs. All inputs optional; `None`
/// yields the platform defaults (Surf Simple pack, `#2563eb` accent,
/// system fonts).
pub fn resolve_theme(
    accent: Option<&str>,
    font: Option<&str>,
    style_pack: Option<&str>,
) -> ResolvedTheme {
    let accent = accent
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .unwrap_or(DEFAULT_ACCENT)
        .to_string();
    let (pack_id, tokens) = resolve_style_pack(style_pack.unwrap_or(DEFAULT_STYLE_PACK));
    let font_stack = font.and_then(resolve_font_preset).map(|p| p.stack.to_string());
    ResolvedTheme {
        on_accent: accent_text_color(&accent).to_string(),
        accent_ink_light: accent_ink_color(&accent, false),
        accent_ink_dark: accent_ink_color(&accent, true),
        accent,
        font_display: font_stack.clone(),
        font_body: font_stack,
        pack_id: pack_id.to_string(),
        tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Surf Simple constants must stay in lockstep with surfdoc.css `:root`.
    /// (render_html's `style_pack_tokens_defined_with_surf_simple_defaults`
    /// pins the CSS side; this pins the Rust side against the same source.)
    #[test]
    fn surf_simple_tokens_match_css_root_defaults() {
        let css = crate::SURFDOC_CSS;
        for (token, value) in [
            ("--ws-radius-card", SURF_SIMPLE_TOKENS.radius_card),
            ("--ws-radius-btn", SURF_SIMPLE_TOKENS.radius_btn),
            ("--ws-radius-chip", SURF_SIMPLE_TOKENS.radius_chip),
            ("--ws-radius-img", SURF_SIMPLE_TOKENS.radius_img),
            ("--ws-border-w", SURF_SIMPLE_TOKENS.border_w),
            ("--ws-border-style", SURF_SIMPLE_TOKENS.border_style),
            ("--ws-shadow-hover", SURF_SIMPLE_TOKENS.shadow_hover),
            ("--ws-bg-texture", SURF_SIMPLE_TOKENS.bg_texture),
            ("--ws-hero-bg", SURF_SIMPLE_TOKENS.hero_bg),
        ] {
            assert!(
                css.contains(&format!("{token}: {value};")),
                "{token} drifted: Rust says `{value}`, surfdoc.css disagrees"
            );
        }
        // Pin --ws-shadow with the full declaration line (its value could be
        // a substring of other shadow declarations).
        assert!(css.contains(&format!("--ws-shadow: {};", SURF_SIMPLE_TOKENS.shadow)));

        // SS-1 additive tokens: every identity default must equal the
        // surfdoc.css `:root` declaration (the DOG-3 no-op pattern).
        for (token, value) in [
            ("--ws-hero-btn-radius", SURF_SIMPLE_TOKENS.hero_btn_radius),
            ("--ws-banner-btn-radius", SURF_SIMPLE_TOKENS.banner_btn_radius),
            ("--ws-cta-radius", SURF_SIMPLE_TOKENS.cta_radius),
            ("--ws-form-submit-radius", SURF_SIMPLE_TOKENS.form_submit_radius),
            ("--ws-feature-card-radius", SURF_SIMPLE_TOKENS.feature_card_radius),
            ("--ws-feature-card-pad", SURF_SIMPLE_TOKENS.feature_card_pad),
            (
                "--ws-feature-card-hover-transform",
                SURF_SIMPLE_TOKENS.feature_card_hover_transform,
            ),
            ("--ws-feature-card-bg", SURF_SIMPLE_TOKENS.feature_card_bg),
            ("--ws-tile-surface-bg", SURF_SIMPLE_TOKENS.tile_surface_bg),
            ("--ws-post-card-bg", SURF_SIMPLE_TOKENS.post_card_bg),
            ("--ws-post-card-radius", SURF_SIMPLE_TOKENS.post_card_radius),
            ("--ws-pg-card-bg", SURF_SIMPLE_TOKENS.pg_card_bg),
            ("--ws-pg-card-radius", SURF_SIMPLE_TOKENS.pg_card_radius),
            ("--ws-pg-tile-radius", SURF_SIMPLE_TOKENS.pg_tile_radius),
            ("--ws-details-bg", SURF_SIMPLE_TOKENS.details_bg),
            ("--ws-details-radius", SURF_SIMPLE_TOKENS.details_radius),
            ("--ws-form-input-bg", SURF_SIMPLE_TOKENS.form_input_bg),
            ("--ws-doc-page-bg", SURF_SIMPLE_TOKENS.doc_page_bg),
            ("--ws-doc-page-radius", SURF_SIMPLE_TOKENS.doc_page_radius),
            ("--ws-drawer-link-size", SURF_SIMPLE_TOKENS.drawer_link_size),
            ("--ws-drawer-link-weight", SURF_SIMPLE_TOKENS.drawer_link_weight),
        ] {
            assert!(
                css.contains(&format!("{token}: {value};")),
                "{token} drifted: Rust says `{value}`, surfdoc.css disagrees"
            );
        }
        // --ws-control-radius is intentionally NOT declared in `:root`
        // (consumers carry divergent per-element fallbacks); the Rust
        // identity value is the guaranteed-invalid `initial`, which keeps
        // every fallback live even if injected.
        assert_eq!(SURF_SIMPLE_TOKENS.control_radius, "initial");
        assert!(
            !css.contains("--ws-control-radius:"),
            "--ws-control-radius must stay fallback-only in surfdoc.css"
        );
        assert!(css.contains("var(--ws-control-radius, var(--radius-sm))"));
        assert!(css.contains("var(--ws-control-radius, 8px)"));
    }

    /// SS-1: the additive token contract derived from the cloudsurf-website
    /// app.css `:root` audit (2026-07-18). app.css var names are the de-facto
    /// contract — this pins the EXACT additive var-name set (audit set-minus
    /// the pre-existing 10 WsTokens fields; `--ws-radius-img` was in the
    /// audit but already existed, so it is value-drift, not additive). Any
    /// rename/add/drop must update this list AND the WsTokens fields.
    #[test]
    fn ss1_additive_token_var_name_contract() {
        let additive = [
            "--ws-hero-btn-radius",
            "--ws-banner-btn-radius",
            "--ws-cta-radius",
            "--ws-form-submit-radius",
            "--ws-control-radius",
            "--ws-feature-card-radius",
            "--ws-feature-card-pad",
            "--ws-feature-card-hover-transform",
            "--ws-feature-card-bg",
            "--ws-tile-surface-bg",
            "--ws-post-card-bg",
            "--ws-post-card-radius",
            "--ws-pg-card-bg",
            "--ws-pg-card-radius",
            "--ws-pg-tile-radius",
            "--ws-details-bg",
            "--ws-details-radius",
            "--ws-form-input-bg",
            "--ws-doc-page-bg",
            "--ws-doc-page-radius",
            "--ws-drawer-link-size",
            "--ws-drawer-link-weight",
        ];
        assert_eq!(additive.len(), 22, "SS-1 contract is exactly 22 additive tokens");
        let unique: std::collections::BTreeSet<_> = additive.iter().collect();
        assert_eq!(unique.len(), additive.len(), "duplicate var name in contract");
        // Every contract var is real: declared in `:root` or consumed via a
        // per-element fallback in the shipped stylesheet.
        for name in additive {
            assert!(
                crate::SURFDOC_CSS.contains(name),
                "{name} not present in surfdoc.css — contract/stylesheet drift"
            );
        }
    }

    #[test]
    fn unknown_pack_falls_back_to_surf() {
        let (id, tokens) = resolve_style_pack("old-school");
        assert_eq!(id, "surf");
        assert_eq!(tokens, SURF_SIMPLE_TOKENS);
    }

    #[test]
    fn comic_pack_resolves() {
        let (id, tokens) = resolve_style_pack("comic");
        assert_eq!(id, "comic");
        assert_eq!(tokens.border_w, "3px");
        assert_eq!(tokens.shadow, "4px 4px 0 rgba(15, 23, 42, 0.85)");
    }

    #[test]
    fn default_theme_is_surf_simple_with_platform_accent() {
        let t = resolve_theme(None, None, None);
        assert_eq!(t.pack_id, "surf");
        assert_eq!(t.accent, DEFAULT_ACCENT);
        assert_eq!(t.tokens, SURF_SIMPLE_TOKENS);
        assert_eq!(t.on_accent, "#fff");
        assert!(t.font_display.is_none());
    }

    /// The AA property the theme ships with: both inks actually pass 4.5:1
    /// against their target surfaces, for every platform palette accent.
    #[test]
    fn resolved_inks_are_aa_for_platform_palettes() {
        for accent in ["#2563eb", "#0ea5e9", "#10b981", "#f59e0b", "#8b5cf6", "#3b82f6"] {
            let t = resolve_theme(Some(accent), None, None);
            assert!(
                contrast_ratio(&t.accent_ink_light, "#eef1f7") >= 4.5,
                "{accent} light ink fails AA"
            );
            assert!(
                contrast_ratio(&t.accent_ink_dark, "#161616") >= 4.5,
                "{accent} dark ink fails AA"
            );
        }
    }

    #[test]
    fn font_preset_resolves_into_theme() {
        let t = resolve_theme(None, Some("inter"), None);
        assert!(t.font_body.as_deref().unwrap().contains("Inter"));
        // Unknown preset → None (native falls back to system fonts).
        assert!(resolve_theme(None, Some("nope"), None).font_body.is_none());
    }

    /// SS-1: `surf-display` resolves to the self-hosted Surf Display face —
    /// stack present, graceful system fallback, and NO import URL (consumers
    /// self-host the font files; the crate must not emit a Google Fonts URL).
    #[test]
    fn surf_display_font_preset_resolves_without_import() {
        let p = resolve_font_preset("surf-display").expect("surf-display is a preset");
        assert!(p.stack.contains("Surf Display"));
        assert!(p.stack.contains("-apple-system"), "needs a graceful fallback stack");
        assert!(p.import.is_none(), "surf-display must not emit an import URL");
        // And it flows into a resolved theme like any other preset.
        let t = resolve_theme(None, Some("surf-display"), None);
        assert!(t.font_display.as_deref().unwrap().contains("Surf Display"));
        assert!(t.font_body.as_deref().unwrap().contains("Surf Display"));
    }
}
