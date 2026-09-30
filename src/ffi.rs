//! UniFFI surface for native (iOS/macOS/Android) consumers.
//!
//! This module is the single FFI boundary for SurfDoc native rendering.
//! Downstream crates (`surfdoc-render` in surfdoc-mobile, `wavesite-core` in
//! wavesite-ios) link this crate with `features = ["uniffi"]` and re-export
//! the surface; they no longer mirror `NativeBlock` into hand-synced shims.
//!
//! Exported types live in [`crate::render_native`] (`NativeBlock` + record
//! structs, annotated with `uniffi::Enum`/`uniffi::Record` behind this
//! feature). Bindings are generated in library mode from any downstream
//! cdylib/staticlib that links us, so the generated Swift/Kotlin module for
//! this crate is named `surf_parse`.

use crate::render_native::{self, NativeBlock, NativeDoc, NativeTheme, NATIVE_DOC_SCHEMA_VERSION};
use crate::resolve;

// ═══════════════════════════════════════════════════════════════════════
// Error
// ═══════════════════════════════════════════════════════════════════════

/// FFI-safe parse error. Name and variants are kept identical to the
/// historical downstream shims so generated Swift (`SurfDocError`) and call
/// sites do not churn.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SurfDocError {
    #[error("Parse error: {msg}")]
    Parse { msg: String },

    #[error("Frontmatter invalid: {msg}")]
    FrontmatterInvalid { msg: String },

    #[error("Internal error: {msg}")]
    Internal { msg: String },
}

// ═══════════════════════════════════════════════════════════════════════
// FFI entry points
// ═══════════════════════════════════════════════════════════════════════

/// Parse a `.surf` source string into a flat list of native blocks.
///
/// Never panics, and FAILS OPEN (0.33.0) exactly like the HTML renderer:
/// the blocks come back whatever diagnostics the source earns — a broken
/// front-matter line or a `::route` without `path=` still renders the
/// body. A client that wants to say "rendered with N problems" reads them
/// from [`surfdoc_diagnostics`]. The `SurfDocError` variants stay in the
/// signature (and the enum) so existing bindings keep compiling.
#[uniffi::export]
pub fn parse_surfdoc(source: String) -> Result<Vec<NativeBlock>, SurfDocError> {
    let doc = parse_checked(&source)?;
    Ok(render_native::to_native_blocks(&doc))
}

/// Parse a `.surf` source string into a [`NativeDoc`]: block tree + resolved
/// theme + schema version.
///
/// Theme inputs come from the document itself (`::site` accent/font and a
/// `style_pack` style property when present); absent inputs resolve to the
/// platform defaults (Surf Simple pack, `#2563eb` accent, system fonts).
/// Hosts that know the site's stored pack/accent (e.g. wavesite-ios reading
/// the sites table) should use [`parse_to_native_styled`] instead.
#[uniffi::export]
pub fn parse_to_native(source: String) -> Result<NativeDoc, SurfDocError> {
    parse_to_native_styled(source, None, None, None)
}

/// [`parse_to_native`] with host-supplied theme inputs. Explicit arguments
/// win over document-derived values; `None` falls through to the document,
/// then to platform defaults. Unknown pack keys resolve to Surf Simple so a
/// stale stored value can never break a render.
#[uniffi::export]
pub fn parse_to_native_styled(
    source: String,
    style_pack: Option<String>,
    accent: Option<String>,
    font: Option<String>,
) -> Result<NativeDoc, SurfDocError> {
    let doc = parse_checked(&source)?;

    // Document-derived theme inputs from the ::site block, if any.
    let (site, _pages, _loose) = crate::render_html::extract_site(&doc);
    let doc_accent = site.as_ref().and_then(|s| s.accent.clone());
    let doc_font = site.as_ref().and_then(|s| s.font.clone());
    let doc_pack = site.as_ref().and_then(|s| {
        s.properties
            .iter()
            .find(|p| p.key == "style_pack" || p.key == "style")
            .map(|p| p.value.clone())
    });

    // Document-derived theme inputs from `::style` blocks (D-S8-5, 0.22.0).
    // Precedence: host argument, then `::style`, then `::site`, then the
    // platform default — `::style` is the more specific override of the two
    // document sources, exactly as its CSS wins in `render_html`.
    let style = resolve::style_theme_inputs(&doc.blocks);

    let theme = resolve::resolve_theme_with_fonts(
        accent.or(style.accent).or(doc_accent).as_deref(),
        font.or(style.font.clone()).or(doc_font).as_deref(),
        style.heading_font.as_deref(),
        style.body_font.as_deref(),
        style_pack.or(doc_pack).as_deref(),
    );

    Ok(NativeDoc {
        schema_version: NATIVE_DOC_SCHEMA_VERSION,
        theme: NativeTheme::from(&theme),
        blocks: render_native::to_native_blocks(&doc),
        block_meta: render_native::to_native_block_meta(&doc),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Size-class axis (schema v5)
// ═══════════════════════════════════════════════════════════════════════

/// Minimum viewport width (logical pt/dp/css-px at 1x) for the `tablet`
/// size class.
///
/// UniFFI 0.28 cannot export a plain constant, so the two breakpoints cross
/// as functions. Clients MUST read them here rather than hard-coding a
/// second breakpoint table — resolution happens once, in Rust.
#[uniffi::export]
pub fn size_class_tablet_min() -> u32 {
    crate::types::SIZE_CLASS_TABLET_MIN
}

/// Minimum viewport width (logical pt/dp/css-px at 1x) for the `desktop`
/// size class. See [`size_class_tablet_min`].
#[uniffi::export]
pub fn size_class_desktop_min() -> u32 {
    crate::types::SIZE_CLASS_DESKTOP_MIN
}

/// Resolve a viewport width to its size-class token ("mobile", "tablet",
/// "desktop"). Total — every width resolves, nothing throws.
#[uniffi::export]
pub fn resolve_size_class(width: u32) -> String {
    resolve::resolve_size_class(width).as_str().to_string()
}

/// Shared parse + fatal-diagnostic routing for the FFI entry points.
// ═══════════════════════════════════════════════════════════════════════
// Block edits by id (0.28.0) — `crate::edit` over the FFI
// ═══════════════════════════════════════════════════════════════════════

/// Every block of `source` with its `id=`, route, depth and span, as a JSON
/// array of `BlockRef` — what a kit shows beside a rendered page and what a
/// native editor addresses an edit with.
#[uniffi::export]
pub fn surfdoc_list_blocks(source: String) -> String {
    crate::edit::list_blocks_json(&source)
}

/// Apply ONE edit (a JSON `EditOp`: `replace` · `insert_after` · `remove` ·
/// `move` · `set_attr` · `set_site_key` · `set_text` · `replace_text` · `stamp_ids`) to
/// `source` and return the new source. An unknown id, route or op is an
/// error — never a silent no-op.
#[uniffi::export]
pub fn surfdoc_apply_edit(source: String, op_json: String) -> Result<String, SurfDocError> {
    crate::edit::apply_json(&source, &op_json).map_err(|e| SurfDocError::Parse { msg: e.to_string() })
}

/// Where a phrase the person quoted occurs (0.29.0): every hit with its
/// block, slot and whether it is verbatim, plus the ONE ambiguity policy's
/// verdict against `current_route` — JSON
/// `{"hits":[…],"policy":"exact|normalized|current_route|ambiguous|none","pick":hit|null}`.
/// What a native client resolves a composer pill with BEFORE it sends.
#[uniffi::export]
pub fn surfdoc_find_text(source: String, query: String, current_route: Option<String>) -> String {
    crate::edit::find_text_json(&source, &query, current_route.as_deref())
}

/// Every diagnostic `source` earns — the parse, schema and lint layers
/// (`crate::lint::check`) — as a JSON array of
/// `{"severity","code","message","line","column"}` (1-based line and
/// column, `null` when the diagnostic has no position; `code` `null` when
/// it has none). What a kit draws its "rendered with N problems" banner
/// from, over the blocks [`parse_to_native`] returned anyway (0.33.0).
#[uniffi::export]
pub fn surfdoc_diagnostics(source: String) -> String {
    diagnostics_json(&source)
}

fn diagnostics_json(source: &str) -> String {
    // Spans index the CRLF-normalised text the parser saw.
    let normalised = source.replace("\r\n", "\n");
    let report = crate::lint::check(source);
    let rows: Vec<serde_json::Value> = report
        .diagnostics
        .iter()
        .map(|d| {
            let (line, column) = match d.span {
                Some(span) if span.start_line > 0 => {
                    let offset = span.start_offset.min(normalised.len());
                    let line_start = normalised
                        .get(..offset)
                        .and_then(|head| head.rfind('\n'))
                        .map_or(0, |i| i + 1);
                    let column = normalised
                        .get(line_start..offset)
                        .map_or(0, |seg| seg.chars().count())
                        + 1;
                    (Some(span.start_line), Some(column))
                }
                _ => (None, None),
            };
            serde_json::json!({
                "severity": d.severity,
                "code": d.code,
                "message": d.message,
                "line": line,
                "column": column,
            })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// The FFI's one parse. Fails open (0.33.0): `crate::parse` always returns
/// a best-effort document — error-severity diagnostics included — and the
/// web renders that document, so the native path does too. Before 0.33.0 an
/// error diagnostic (an unquoted colon in a front-matter value) refused the
/// whole document here while the web showed it. The `Result` stays so the
/// exported signatures do not change.
fn parse_checked(source: &str) -> Result<crate::types::SurfDoc, SurfDocError> {
    Ok(crate::parse(source).doc)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests (migrated from the downstream shims when the FFI moved upstream)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// The breakpoint pair is the whole client contract: it must cross the
    /// FFI, and it must equal the Rust constants (schema v5).
    #[test]
    fn breakpoint_constants_cross_the_ffi() {
        assert_eq!(size_class_tablet_min(), crate::types::SIZE_CLASS_TABLET_MIN);
        assert_eq!(size_class_desktop_min(), crate::types::SIZE_CLASS_DESKTOP_MIN);
        assert_eq!(size_class_tablet_min(), 768);
        assert_eq!(size_class_desktop_min(), 1024);
        assert_eq!(resolve_size_class(767), "mobile");
        assert_eq!(resolve_size_class(768), "tablet");
        assert_eq!(resolve_size_class(1023), "tablet");
        assert_eq!(resolve_size_class(1024), "desktop");
    }

    /// The schema number is what tells a client the new variants are present.
    /// (This pin still read 12 after 0.32.0 moved the schema to 13; the
    /// uniffi feature is outside the default gates, so 0.33.0 moves it to the
    /// current number.)
    #[test]
    fn native_doc_schema_version_is_fourteen() {
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 14);
        let doc = parse_to_native("# Hi\n".into()).expect("parse");
        assert_eq!(doc.schema_version, 14);
    }

    /// 0.33.0 fail-open: a front-matter value with an unquoted colon is
    /// broken YAML (P002, an error) — the body still crosses the FFI, and the
    /// diagnostics JSON names the problem with its position.
    #[test]
    fn broken_front_matter_still_renders_and_names_p002() {
        let source = "---\ntitle: Plan\nupdated: 2026-09-29 10:30: late\n---\n\n# Body\n\nStill here.\n";
        let blocks = parse_surfdoc(source.into()).expect("fails open");
        assert!(
            blocks.iter().any(|b| matches!(b, NativeBlock::Markdown { content } if content.contains("Still here"))),
            "{blocks:?}"
        );
        let doc = parse_to_native(source.into()).expect("fails open");
        assert!(!doc.blocks.is_empty());
        let styled = parse_to_native_styled(source.into(), None, None, None).expect("fails open");
        assert_eq!(styled.blocks.len(), doc.blocks.len());

        let diags: serde_json::Value = serde_json::from_str(&surfdoc_diagnostics(source.into())).expect("json");
        let p002 = diags
            .as_array()
            .expect("array")
            .iter()
            .find(|d| d["code"] == "P002")
            .unwrap_or_else(|| panic!("P002 missing: {diags}"));
        assert_eq!(p002["severity"], "error");
        assert!(p002["message"].as_str().is_some_and(|m| !m.is_empty()));
        assert!(p002["line"].as_u64().is_some_and(|l| l >= 1), "{p002}");
        assert!(p002["column"].as_u64().is_some_and(|c| c >= 1), "{p002}");
    }

    /// A `::route` without `path=` (V310, an error) still crosses as a
    /// Route, and the diagnostics JSON names V310 on the route's line.
    #[test]
    fn route_without_path_renders_and_names_v310() {
        let source = "# API\n\n::route[method=GET]\nreturns: list(Entry)\n::\n";
        let blocks = parse_surfdoc(source.into()).expect("fails open");
        assert!(blocks.iter().any(|b| matches!(b, NativeBlock::Route { .. })), "{blocks:?}");
        let diags: serde_json::Value = serde_json::from_str(&surfdoc_diagnostics(source.into())).expect("json");
        let v310 = diags
            .as_array()
            .expect("array")
            .iter()
            .find(|d| d["code"] == "V310")
            .unwrap_or_else(|| panic!("V310 missing: {diags}"));
        assert_eq!(v310["severity"], "error");
        assert_eq!(v310["line"], 3);
        assert_eq!(v310["column"], 1);
    }

    /// A clean document earns an empty diagnostics array, not `null`.
    #[test]
    fn clean_document_has_no_error_diagnostics() {
        let json = surfdoc_diagnostics("---\ntitle: Clean\ntype: doc\n---\n\n# Hi\n".into());
        let diags: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert!(diags.is_array());
        assert!(
            diags.as_array().unwrap().iter().all(|d| d["severity"] != "error"),
            "{diags}"
        );
    }

    /// v11: a section headline's `{#slug}` crosses as `anchor`, never as text.
    #[test]
    fn section_anchor_crosses_the_ffi() {
        let doc = parse_to_native("::section[bg=story]\n## Three generations. {#story}\n\nBody.\n::\n".into())
            .expect("parse");
        match &doc.blocks[0] {
            NativeBlock::SectionContainer { headline, anchor, .. } => {
                assert_eq!(headline.as_deref(), Some("Three generations."));
                assert_eq!(anchor.as_deref(), Some("story"));
            }
            other => panic!("expected SectionContainer, got {other:?}"),
        }
    }

    /// v6 addressing: `id=`/`label=` reach the FFI as a span-indexed list.
    #[test]
    fn block_addressing_crosses_the_ffi() {
        let doc = parse_to_native(
            "::callout[type=info id=intro label=\"Read me\"]\nHi.\n::\n".into(),
        )
        .expect("parse");
        assert_eq!(doc.block_meta.len(), 1);
        let meta = &doc.block_meta[0];
        assert_eq!(meta.block_id.as_deref(), Some("intro"));
        assert_eq!(meta.label.as_deref(), Some("Read me"));
        assert_eq!(meta.start_line, 1);
        assert!(meta.end_offset > meta.start_offset);

        // A document that authors neither carries an empty list.
        let plain = parse_to_native("# Hi\n".into()).expect("parse");
        assert!(plain.block_meta.is_empty());
    }

    #[test]
    fn happy_path_heading_paragraph_code() {
        let blocks = parse_surfdoc(
            "# Title\n\nSome paragraph.\n\n::code[lang=rust]\nfn main() {}\n::\n".into(),
        )
        .expect("parse should succeed");
        assert!(blocks.len() >= 2);
        assert!(blocks.iter().any(|b| matches!(b, NativeBlock::Code { .. })));
    }

    #[test]
    fn frontmatter_only_doc() {
        let blocks = parse_surfdoc("---\ntitle: Test\n---\n".into())
            .expect("frontmatter-only doc should parse");
        assert!(blocks.is_empty());
    }

    #[test]
    fn plain_paragraph_emits_markdown_block() {
        let blocks = parse_surfdoc("Just text.".into()).expect("parse should succeed");
        assert!(matches!(&blocks[0], NativeBlock::Markdown { content } if content.contains("Just text")));
    }

    #[test]
    fn hero_directive_roundtrip() {
        let blocks = parse_surfdoc("::hero\nheadline: Welcome\nsubtitle: to SurfDoc\n::\n".into())
            .expect("parse should succeed");
        assert!(blocks
            .iter()
            .any(|b| matches!(b, NativeBlock::Hero { .. })));
    }

    #[test]
    fn parse_to_native_defaults_to_surf_simple() {
        let doc = parse_to_native("# Hello\n".into()).expect("parse");
        assert_eq!(doc.schema_version, NATIVE_DOC_SCHEMA_VERSION);
        assert_eq!(doc.theme.pack_id, "surf");
        assert_eq!(doc.theme.accent, crate::resolve::DEFAULT_ACCENT);
        assert_eq!(doc.theme.radius_card, 16.0);
        assert_eq!(doc.theme.radius_btn, 10.0);
        assert!(!doc.blocks.is_empty());
    }

    #[test]
    fn parse_to_native_styled_overrides_win() {
        let source = "::site\nname: Demo\naccent: #10b981\n::\n# Hi\n";
        let doc = parse_to_native_styled(source.into(), Some("comic".into()), None, None)
            .expect("parse");
        assert_eq!(doc.theme.pack_id, "comic");
        assert_eq!(doc.theme.border_w, 3.0);
        assert_eq!(doc.theme.radius_card, 4.0);
        // Doc-derived accent survives a pack override.
        assert_eq!(doc.theme.accent, "#10b981");
        // Derived colors are computed, not defaulted.
        assert!(crate::resolve::contrast_ratio(&doc.theme.accent_ink_light, "#eef1f7") >= 4.5);
    }

    #[test]
    fn parse_to_native_unknown_pack_falls_back() {
        let doc = parse_to_native_styled("# Hi\n".into(), Some("old-school".into()), None, None)
            .expect("parse");
        assert_eq!(doc.theme.pack_id, "surf");
    }

    #[test]
    fn section_preserves_children() {
        let blocks = parse_surfdoc(
            "::section[headline=\"S\"]\nInner text\n::\n".into(),
        )
        .expect("parse should succeed");
        assert!(blocks.iter().any(
            |b| matches!(b, NativeBlock::SectionContainer { children, .. } if !children.is_empty())
        ));
    }
}
