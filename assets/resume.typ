// SurfDoc Typst RESUME profile — the one-page US-Letter resume (surf-parse 0.31.1,
// TASK-1074 lane R; the spacing pass of 2026-09-28). The numbers are the measured V8
// build the profile reproduces (cloudsurf-strategy-staging/.context/guides/
// resume-pdf-from-linkedin-html-headless-chrome.surf §3): Letter · 0.42 / 0.65 / 0.3 /
// 0.65 in · Inter 9.75 pt at a 1.3 line pitch · a 2 pt header rule · 8.5 pt uppercase
// 0.14 em section titles over a 1 pt rule · role left, date right · entries that never
// split · the last two sections side by side.
//
// THE SPACING LAW (2026-09-28). Typst measures a line from its cap height to its
// BASELINE, Chrome from the top of the line box to its bottom: every CSS gap between
// two lines is therefore written here as gap + 0.287 em of the line above + 0.287 em
// of the line below (Inter's descent 0.242 + the half-leading of a 1.3 pitch). A
// block's weak `above`/`below` spacing collapsed against its neighbour in the 0.31.0
// build (sections butted, entries touched), so every gap is a `gap(h)` SPACER BLOCK
// the template or the generator writes, and every content block carries 0 pt of
// weak spacing. Leading 0.575 em = the 1.3 pitch over Inter's 0.727 em cap height.
// Embedded via include_str!() and prepended to the generated markup. The page's
// paper comes from the PDF config (the doc's front matter wins); only the margins
// are stated here.

#set page(margin: (top: 0.42in, right: 0.65in, bottom: 0.3in, left: 0.65in))
#set text(font: ("Inter", "Liberation Sans"), size: 9.75pt, fill: rgb("#1c2430"))
#set par(justify: false, leading: 0.575em, spacing: 10.5pt)
#set list(marker: text(fill: rgb("#7b8794"))[•], indent: 3pt, body-indent: 6pt, spacing: 6.6pt)
#set block(above: 0pt, below: 0pt)
#show link: set text(fill: rgb("#1c2430"))

// A spacer: the one way a gap is written (never a weak spacing).
#let gap(h) = block(height: h, width: 100%, above: 0pt, below: 0pt)

// The head: the name, the headline, the contact line, closed by the 2 pt rule.
#let resume-head(name, headline, contact) = block(
  width: 100%, stroke: (bottom: 2pt + rgb("#1c2430")), inset: (bottom: 10.6pt), above: 0pt, below: 0pt,
)[
  #text(size: 22pt, weight: 700, tracking: -0.015em)[#name]
  #if headline != none [
    #gap(10.4pt)
    #text(size: 11.5pt, weight: 500, fill: rgb("#3a4656"))[#headline]
  ]
  #if contact != none [
    #gap(12pt)
    #text(size: 9.25pt, fill: rgb("#4a5568"))[#contact]
  ]
]

// A section: the uppercase spaced title over a 1 pt rule, then its body.
#let resume-section(title, body) = block(width: 100%, above: 0pt, below: 0pt)[
  #block(width: 100%, stroke: (bottom: 1pt + rgb("#d5dae1")), inset: (bottom: 5.4pt), above: 0pt, below: 0pt)[
    #text(size: 8.5pt, weight: 700, tracking: 0.14em)[#upper(title)]
  ]
  #gap(8.8pt)
  #body
]

// An entry: role — org on the left, the date right-aligned in lining figures,
// a muted where-line, then the bullets. Never splits across pages.
#let resume-entry(role, org, when, where, body) = block(width: 100%, breakable: false, above: 0pt, below: 0pt)[
  #grid(
    columns: (1fr, auto), column-gutter: 12pt, align: (left + bottom, right + bottom),
    [
      #text(size: 10.25pt, weight: 700)[#role]#if org != none [#text(fill: rgb("#9aa4b1"))[ — ]#text(weight: 500, fill: rgb("#3a4656"))[#org]]
    ],
    [#if when != none [#text(size: 9.25pt, fill: rgb("#5a6675"), number-type: "lining")[#when]]],
  )
  #if where != none [
    #gap(6.6pt)
    #text(size: 9.25pt, fill: rgb("#6b7684"))[#where]
    #gap(8.5pt)
  ] else [
    #gap(8.7pt)
  ]
  #body
]

// A credential: the bold line and its muted sub-line.
#let resume-cert(main, sub) = block(above: 0pt, below: 0pt)[
  #text(weight: 600)[#main]
  #if sub != none [
    #linebreak()
    #text(size: 9.25pt, fill: rgb("#5a6675"))[#sub]
  ]
]

// A skill group: "Group: a · b · c".
#let resume-skill(group, items) = block(above: 0pt, below: 0pt)[
  #text(weight: 600)[#group:] #text(fill: rgb("#3a4656"))[#items]
]

// The two short bottom sections side by side.
#let resume-cols(left, right) = grid(columns: (1fr, 1fr), column-gutter: 26pt, left, right)

// The grey separator of the contact line.
#let resume-sep = text(fill: rgb("#9aa4b1"))[ · ]
