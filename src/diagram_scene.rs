//! Typed geometry scenes for `::diagram` blocks.
//!
//! A [`NativeDiagramScene`] is the resolved *layout* of a diagram: absolute
//! coordinates, sizes and text runs, with every paintable surface expressed
//! as a semantic [`NativeRole`] instead of a concrete color. Layout runs once
//! in Rust; each consumer then draws the same scene with platform styling —
//! the built-in SVG renderer maps roles to its fixed palette, native clients
//! map them to their own theme tokens.
//!
//! Every type here is FFI-safe (UniFFI record/enum shapes: only `String`,
//! `bool`, `f64`, `Option`, `Vec` and simple structs/enums of the same) and
//! serializes with serde for JSON consumers.
//!
//! DETERMINISM: scenes are pure functions of the parsed diagram model. All
//! coordinates are produced by integer layout arithmetic and widened to
//! `f64`, so a given document always yields the identical scene.

use serde::{Deserialize, Serialize};

/// A complete diagram scene: canvas size plus a z-ordered shape list
/// (earlier shapes paint first, i.e. underneath later ones).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeDiagramScene {
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<NativeShape>,
}

/// A 2-D point in scene coordinates (origin top-left, y down).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativePoint {
    pub x: f64,
    pub y: f64,
}

/// Semantic paint role. Consumers resolve each role to a concrete color for
/// their theme; the reference SVG palette is documented per variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum NativeRole {
    /// Default node/box background (SVG: `#f8fafc`).
    Surface,
    /// Emphasized surface — title bars, activation bars, roots, and the
    /// gantt axis gridlines (SVG: `#e2e8f0`).
    SurfaceAlt,
    /// Theme accent, reserved for highlighted shapes (SVG: `#2563eb`;
    /// unused by the built-in diagram types today).
    Accent,
    /// Soft accent — de-emphasized guide lines such as sequence lifelines
    /// (SVG: `#cbd5e1`).
    AccentSoft,
    /// Primary outline/edge color, also arrowheads and state markers
    /// (SVG: `#64748b`).
    Stroke,
    /// Muted solid fill — gantt bars, mindmap branch connectors
    /// (SVG: `#94a3b8`).
    Muted,
    /// Primary text (SVG: `currentColor`, i.e. the page text color).
    TextPrimary,
    /// Secondary text — edge labels, badges, ticks (SVG: `#64748b`).
    TextSecondary,
    /// Paint on top of / inside accent or paper surfaces — also the plain
    /// paper fill of table-style boxes (SVG: `#ffffff`).
    OnAccent,
}

/// Horizontal text anchoring relative to a label's `x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum NativeAnchor {
    Start,
    Middle,
    End,
}

/// Line-end marker kind. `Diamond`/`DiamondOpen`/`TriangleOpen` are the UML
/// composition / aggregation / inheritance markers used by `class` diagrams;
/// all markers point along the line direction at their endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum NativeMarker {
    None,
    Arrow,
    Diamond,
    DiamondOpen,
    TriangleOpen,
}

/// One paintable element of a diagram scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeShape {
    /// Axis-aligned rectangle. `corner` is the corner radius (0 = square).
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        corner: f64,
        fill: NativeRole,
        stroke: NativeRole,
        stroke_width: f64,
    },
    /// Open polyline through `points` (2 points = straight segment). Never
    /// filled. `dashed` marks async/guide styling; markers sit on the first
    /// and last point respectively.
    Line {
        points: Vec<NativePoint>,
        stroke: NativeRole,
        stroke_width: f64,
        dashed: bool,
        marker_start: NativeMarker,
        marker_end: NativeMarker,
    },
    /// Closed filled polygon (e.g. flowchart diamonds).
    Polygon {
        points: Vec<NativePoint>,
        fill: NativeRole,
        stroke: NativeRole,
    },
    /// Ellipse/circle. `fill: None` paints a hollow ring; `stroke: None`
    /// draws no outline.
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        fill: Option<NativeRole>,
        stroke: Option<NativeRole>,
    },
    /// A text run. `y` is the text baseline; `size` is the font size in
    /// scene units (13 = the diagram base size).
    Label {
        x: f64,
        y: f64,
        text: String,
        role: NativeRole,
        size: f64,
        bold: bool,
        mono: bool,
        anchor: NativeAnchor,
    },
}
