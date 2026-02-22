// SurfDoc Typst Template — base page setup, colors, and reusable functions.
// This file is embedded via include_str!() and prepended to generated Typst markup.

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 2.5cm, left: 2cm, right: 2cm),
  header: context {
    if counter(page).get().first() > 1 [
      #set text(size: 8pt, fill: luma(150))
      #h(1fr) SurfDoc
    ]
  },
  footer: context [
    #set text(size: 8pt, fill: luma(150))
    #h(1fr) #counter(page).display("1 / 1", both: true) #h(1fr)
  ],
)

#set text(font: "Libertinus Serif", size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: none)

#show heading.where(level: 1): set text(size: 1.5em, weight: "bold")
#show heading.where(level: 2): set text(size: 1.25em, weight: "bold")
#show heading.where(level: 3): set text(size: 1.1em, weight: "bold")

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
  block(
    fill: colors.bg,
    inset: 12pt,
    radius: 4pt,
    stroke: (left: 3pt + colors.border),
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
