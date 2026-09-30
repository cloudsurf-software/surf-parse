// SurfDoc Typst Template — base page setup, colors, and reusable functions.
// This file is embedded via include_str!() and prepended to generated Typst markup.

// The paper is NOT set here: the PDF config's `#set page(paper: …)` override precedes this template and must
// win (a doc's `paper: letter` and the route's Letter came out A4 until 2026-09-28); with no override Typst's
// own default (A4) applies, as before. The page FURNITURE is not set here either (surf-parse 0.34.0): the running
// footer — the centred page counter, and the caller's brand words bottom-right when it asks for them — is written
// by the renderer's `build_page_furniture` before this template, and a `#set page` merges per property, so this
// margin rule leaves it standing. The "SurfDoc" running head that sat top-right of every page after the first is
// retired: nothing in the engine names itself on a page.
#set page(margin: (top: 2.5cm, bottom: 2.5cm, left: 2cm, right: 2cm))

// Body font mirrors the on-screen surfdoc viewer (system sans-serif). Liberation
// Sans is bundled by the renderer; raw/code falls back to DejaVu Sans Mono.
#set text(font: "Liberation Sans", size: 11pt, fill: rgb("#0f1422"))
#set par(justify: false, leading: 0.7em, spacing: 1.1em)
#set heading(numbering: none)

#show heading.where(level: 1): set text(size: 1.6em, weight: "bold")
#show heading.where(level: 2): set text(size: 1.33em, weight: "bold")
#show heading.where(level: 3): set text(size: 1.2em, weight: "bold")
#show heading.where(level: 4): set text(size: 1.05em, weight: "bold")

#show raw: set text(font: "DejaVu Sans Mono")
#show raw.where(block: true): set text(size: 9pt)
#show raw.where(block: true): block.with(
  fill: luma(245),
  inset: 10pt,
  radius: 4pt,
  width: 100%,
)
#show raw.where(block: false): box.with(
  fill: luma(240),
  inset: (x: 3pt, y: 0pt),
  outset: (y: 3pt),
  radius: 2pt,
)

// --- Color definitions ---

#let surfdoc-blue = rgb("#3b82f6")
#let surfdoc-green = rgb("#22c55e")
#let surfdoc-yellow = rgb("#eab308")
#let surfdoc-red = rgb("#ef4444")
#let surfdoc-orange = rgb("#f97316")
#let surfdoc-purple = rgb("#a855f7")
#let surfdoc-gray = luma(100)

#let callout-colors = (
  info: (bg: rgb("#eff6ff"), border: surfdoc-blue, text: rgb("#1e40af")),
  warning: (bg: rgb("#fffbeb"), border: surfdoc-yellow, text: rgb("#92400e")),
  danger: (bg: rgb("#fef2f2"), border: surfdoc-red, text: rgb("#991b1b")),
  tip: (bg: rgb("#f0fdf4"), border: surfdoc-green, text: rgb("#166534")),
  note: (bg: rgb("#f5f3ff"), border: surfdoc-purple, text: rgb("#5b21b6")),
  success: (bg: rgb("#f0fdf4"), border: surfdoc-green, text: rgb("#166534")),
)

#let decision-colors = (
  proposed: surfdoc-blue,
  accepted: surfdoc-green,
  rejected: surfdoc-red,
  superseded: surfdoc-gray,
)

// --- Reusable components ---

#let surfdoc-callout(type-name, title, body) = {
  let colors = callout-colors.at(type-name, default: callout-colors.info)
  // A tinted card, 6pt corners, NO stroke on any side (0.34.0): the one-sided accent bar is not a card language
  // the on-screen doc has, and the kind still reads from the tint and the title colour.
  block(
    fill: colors.bg,
    inset: 12pt,
    radius: 6pt,
    width: 100%,
  )[
    #set text(fill: colors.text)
    #if title != none [
      #text(weight: "bold")[#title] \
    ] else [
      #text(weight: "bold")[#upper(type-name)] \
    ]
    #body
  ]
}

#let surfdoc-decision-badge(status) = {
  let color = decision-colors.at(status, default: surfdoc-gray)
  box(
    fill: color,
    radius: 2pt,
    inset: (x: 6pt, y: 2pt),
  )[#text(fill: white, size: 9pt, weight: "bold")[#upper(status)]]
}

#let surfdoc-metric(label, value, unit: none, trend: none) = {
  let trend-symbol = if trend == "up" { sym.arrow.t }
    else if trend == "down" { sym.arrow.b }
    else { "" }
  align(center)[
    #text(size: 2em, weight: "bold")[#value#if unit != none [ #text(size: 0.5em)[#unit]]]
    #if trend != none [ #trend-symbol]
    \
    #text(fill: luma(100))[#label]
  ]
}
