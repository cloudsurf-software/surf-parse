//! Client DoS bounds for the SurfDoc web runtime — `max_depth`,
//! `max_blocks`, `max_source_bytes` (spec/web-runtime-v1.surf §4.4).
//!
//! The bounds are a *reject*, never a sanitize: a document that blows past
//! one is refused whole (publish-side reject on the server, coverage decline
//! on the client), so both sides agree on exactly which documents exist.
//! Enforcement is fuel-shaped — the walk aborts at the first violation
//! instead of measuring a hostile tree to completion.
//!
//! This module is NOT behind the `dom` feature: the server publish path needs
//! the same numbers the client runtime enforces, and it does not build the
//! constructive DOM backend. The `dom` backend re-exports the bounds through
//! [`crate::render_dom::RenderDomError::LimitExceeded`] so the takeover gate
//! returns one typed decline for coverage and bounds alike.

use crate::types::{Block, SurfDoc};

/// Which bound a document blew past, and by how much the walk had counted
/// when it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LimitExceeded {
    /// Block nesting went past `max_depth`. `reached` is the depth of the
    /// offending block (top-level blocks are depth 1).
    #[error("nesting depth {reached} exceeds max_depth {limit}")]
    Depth { limit: usize, reached: usize },
    /// The tree holds more blocks than `max_blocks`. `reached` is the count
    /// at which the walk aborted (`limit + 1`), not the full tree size — the
    /// walk deliberately stops rather than measuring a hostile document.
    #[error("block count reached {reached}, exceeding max_blocks {limit}")]
    Blocks { limit: usize, reached: usize },
    /// The source text is larger than `max_source_bytes`. Checked before any
    /// parsing happens, so it is the cheapest of the three.
    #[error("source is {reached} bytes, exceeding max_source_bytes {limit}")]
    SourceBytes { limit: usize, reached: usize },
}

/// Parse/render bounds applied identically at publish and at parse.
///
/// [`ParseLimits::default()`] carries the shipped numbers; construct the
/// struct literally to tighten them per surface. Every field is an inclusive
/// maximum — a value equal to the limit passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Maximum block nesting depth. Top-level blocks sit at depth 1, a block
    /// inside a `::page` at depth 2, and so on.
    pub max_depth: usize,
    /// Maximum total number of blocks in the tree, nested blocks included.
    pub max_blocks: usize,
    /// Maximum size of the SurfDoc source text, in bytes (not characters).
    pub max_source_bytes: usize,
}

impl ParseLimits {
    /// Default nesting depth cap.
    ///
    /// Measured 2026-08-26 over the 128 `.surf` fixtures in `tests/fixtures`:
    /// the deepest vendored Surfspace web-shell surface nests 4 levels, and
    /// the deepest tree in the whole tree — the hostile max-nesting chrome
    /// fixture — reaches 11. 64 leaves ample headroom for generated shells
    /// (app-shell → tab-content → split-pane → pane → app-shell chains) while
    /// keeping the recursive walks stack-safe.
    pub const DEFAULT_MAX_DEPTH: usize = 64;

    /// Default cap on total block count. The largest fixture in the corpus
    /// holds 58 blocks (measured 2026-08-26), so real documents never see
    /// this, while a generated block-bomb is refused before it reaches the
    /// DOM.
    pub const DEFAULT_MAX_BLOCKS: usize = 20_000;

    /// Default cap on source size: 2 MiB. The largest vendored web-shell
    /// source is 21,363 bytes (measured 2026-08-26) and the wasm bundle gate
    /// itself sits at 1.2 MB gz — a source past 2 MiB is a payload, not a
    /// page.
    pub const DEFAULT_MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

    /// Bounds that never fire. For tests and for callers that have already
    /// bounded their input some other way; not for untrusted documents.
    pub const fn unlimited() -> Self {
        Self {
            max_depth: usize::MAX,
            max_blocks: usize::MAX,
            max_source_bytes: usize::MAX,
        }
    }

    /// Source-size bound. Call this BEFORE parsing — it is the gate that
    /// keeps a hostile payload from being parsed at all.
    pub fn check_source_bytes(&self, source: &str) -> Result<(), LimitExceeded> {
        let len = source.len();
        if len > self.max_source_bytes {
            return Err(LimitExceeded::SourceBytes {
                limit: self.max_source_bytes,
                reached: len,
            });
        }
        Ok(())
    }

    /// Depth and block-count bounds over a parsed tree. One fuel-shaped
    /// walk: it returns at the first block that violates either bound.
    pub fn check_blocks(&self, blocks: &[Block]) -> Result<(), LimitExceeded> {
        let mut counted = 0usize;
        self.walk(blocks, 1, &mut counted)
    }

    /// [`ParseLimits::check_blocks`] over a whole document.
    pub fn check_doc(&self, doc: &SurfDoc) -> Result<(), LimitExceeded> {
        self.check_blocks(&doc.blocks)
    }

    fn walk(
        &self,
        blocks: &[Block],
        depth: usize,
        counted: &mut usize,
    ) -> Result<(), LimitExceeded> {
        if !blocks.is_empty() && depth > self.max_depth {
            return Err(LimitExceeded::Depth {
                limit: self.max_depth,
                reached: depth,
            });
        }
        for block in blocks {
            *counted += 1;
            if *counted > self.max_blocks {
                return Err(LimitExceeded::Blocks {
                    limit: self.max_blocks,
                    reached: *counted,
                });
            }
            let (a, b) = child_slices(block);
            self.walk(a, depth + 1, counted)?;
            self.walk(b, depth + 1, counted)?;
        }
        Ok(())
    }
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_depth: Self::DEFAULT_MAX_DEPTH,
            max_blocks: Self::DEFAULT_MAX_BLOCKS,
            max_source_bytes: Self::DEFAULT_MAX_SOURCE_BYTES,
        }
    }
}

/// The nested-block slices a container owns. Every `Vec<Block>` field in
/// [`Block`] is represented here; [`Block::SplitPane`] is the only variant
/// with two. Row/toolbar items are item structs, not blocks, so they carry
/// no depth of their own.
fn child_slices(block: &Block) -> (&[Block], &[Block]) {
    const NONE: &[Block] = &[];
    match block {
        Block::Page { children, .. }
        | Block::Slide { children, .. }
        | Block::Section { children, .. }
        | Block::App { children, .. }
        | Block::AppShell { children, .. }
        | Block::Sidebar { children, .. }
        | Block::Panel { children, .. }
        | Block::TabContent { children, .. }
        | Block::Drawer { children, .. }
        | Block::Modal { children, .. } => (children.as_slice(), NONE),
        Block::SplitPane { left, right, .. } => (left.as_slice(), right.as_slice()),
        _ => (NONE, NONE),
    }
}

/// Measured nesting depth of a block tree (0 for an empty slice). Reported
/// by the CLI/server when a document is rejected; the enforcement path uses
/// [`ParseLimits::check_blocks`] instead, which never walks a hostile tree
/// to completion.
pub fn measure_depth(blocks: &[Block]) -> usize {
    let mut deepest = 0usize;
    for block in blocks {
        let (a, b) = child_slices(block);
        let below = measure_depth(a).max(measure_depth(b));
        deepest = deepest.max(1 + below);
    }
    deepest
}

/// Measured total block count of a tree, nested blocks included. Same
/// caveat as [`measure_depth`]: measurement, not enforcement.
pub fn measure_blocks(blocks: &[Block]) -> usize {
    let mut total = 0usize;
    for block in blocks {
        total += 1;
        let (a, b) = child_slices(block);
        total += measure_blocks(a) + measure_blocks(b);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_numbers() {
        let l = ParseLimits::default();
        assert_eq!(l.max_depth, 64);
        assert_eq!(l.max_blocks, 20_000);
        assert_eq!(l.max_source_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn measure_matches_a_known_tree() {
        let doc = crate::parse("::page[title=T]\n\n::section[title=S]\n\nhi\n\n::\n\n::\n").doc;
        assert_eq!(measure_depth(&doc.blocks), 3);
        assert_eq!(measure_blocks(&doc.blocks), 3);
        assert!(ParseLimits::default().check_doc(&doc).is_ok());
    }

    #[test]
    fn empty_tree_is_within_every_bound() {
        let l = ParseLimits {
            max_depth: 0,
            max_blocks: 0,
            max_source_bytes: 0,
        };
        assert!(l.check_blocks(&[]).is_ok());
        assert!(l.check_source_bytes("").is_ok());
    }
}
