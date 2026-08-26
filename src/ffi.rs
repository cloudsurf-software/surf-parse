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
/// Never panics. Fatal diagnostics are routed to `FrontmatterInvalid` when
/// they reference YAML/front-matter; all other fatals become `Parse`.
/// Non-fatal diagnostics are dropped at this layer.
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

    let theme = resolve::resolve_theme(
        accent.or(doc_accent).as_deref(),
        font.or(doc_font).as_deref(),
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
fn parse_checked(source: &str) -> Result<crate::types::SurfDoc, SurfDocError> {
    let result = crate::parse(source);

    let fatals: Vec<&crate::error::Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::error::Severity::Error)
        .collect();

    if !fatals.is_empty() {
        let msg = fatals
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        let is_frontmatter = fatals.iter().any(|d| {
            let m = d.message.to_lowercase();
            m.contains("front matter") || m.contains("frontmatter") || m.contains("yaml")
        });
        return Err(if is_frontmatter {
            SurfDocError::FrontmatterInvalid { msg }
        } else {
            SurfDocError::Parse { msg }
        });
    }

    Ok(result.doc)
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

    /// Schema v6 is what tells a client the new fields are present.
    #[test]
    fn native_doc_schema_version_is_six() {
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 6);
        let doc = parse_to_native("# Hi\n".into()).expect("parse");
        assert_eq!(doc.schema_version, 6);
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
