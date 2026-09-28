// SurfDoc Typst RESUME profile — the one-page US-Letter resume (surf-parse 0.31.0,
// TASK-1074 lane R). The numbers are the measured V7 build the profile reproduces
// (cloudsurf-strategy-staging/.context/guides/resume-pdf-from-linkedin-html-headless-chrome.surf §3):
// Letter · 0.42 / 0.65 / 0.3 / 0.65 in · Inter 9.75 pt / 1.3 · a 2 pt header rule ·
// 8.5 pt uppercase 0.14 em section titles over a 1 pt rule · role left, date right ·
// entries that never split · the last two sections side by side.
// Embedded via include_str!() and prepended to the generated markup. The page's
// paper and margins are re-stated by PdfConfig (the doc's front matter wins).

#set page(paper: "us-letter", margin: (top: 0.42in, right: 0.65in, bottom: 0.3in, left: 0.65in))
#set text(font: ("Inter", "Liberation Sans"), size: 9.75pt, fill: rgb("#1c2430"))
#set par(justify: false, leading: 0.5em, spacing: 0.7em)
#set list(marker: text(fill: rgb("#7b8794"))[•], indent: 3pt, body-indent: 6pt, spacing: 0.4em)
#show link: set text(fill: rgb("#1c2430"))

// The head: the name, the headline, the contact line, closed by the 2 pt rule.
#let resume-head(name, headline, contact) = block(
  width: 100%, stroke: (bottom: 2pt + rgb("#1c2430")), inset: (bottom: 8pt), below: 8pt,
)[
  #text(size: 22pt, weight: 700, tracking: -0.015em)[#name]
  #if headline != none [
    #v(3pt, weak: true)
    #text(size: 11.5pt, weight: 500, fill: rgb("#3a4656"))[#headline]
  ]
  #if contact != none [
    #v(6pt, weak: true)
    #text(size: 9.25pt, fill: rgb("#4a5568"))[#contact]
  ]
]

// A section: the uppercase spaced title over a 1 pt rule, then its body.
#let resume-section(title, body) = block(width: 100%, below: 8pt)[
  #block(width: 100%, stroke: (bottom: 1pt + rgb("#d5dae1")), inset: (bottom: 3pt), below: 6pt)[
    #text(size: 8.5pt, weight: 700, tracking: 0.14em)[#upper(title)]
  ]
  #body
]

// An entry: role — org on the left, the date right-aligned in lining figures,
// a muted where-line, then the bullets. Never splits across pages.
#let resume-entry(role, org, when, where, body) = block(width: 100%, breakable: false, below: 6.5pt)[
  #grid(
    columns: (1fr, auto), column-gutter: 12pt, align: (left + bottom, right + bottom),
    [
      #text(size: 10.25pt, weight: 700)[#role]#if org != none [#text(fill: rgb("#9aa4b1"))[ — ]#text(weight: 500, fill: rgb("#3a4656"))[#org]]
    ],
    [#if when != none [#text(size: 9.25pt, fill: rgb("#5a6675"), number-type: "lining")[#when]]],
  )
  #if where != none [
    #v(2.5pt)
    #text(size: 9.25pt, fill: rgb("#6b7684"))[#where]
  ]
  #v(3.5pt)
  #body
]

// A credential: the bold line and its muted sub-line.
#let resume-cert(main, sub) = block(below: 4pt)[
  #text(weight: 600)[#main]
  #if sub != none [
    #linebreak()
    #text(size: 9.25pt, fill: rgb("#5a6675"))[#sub]
  ]
]

// A skill group: "Group: a · b · c".
#let resume-skill(group, items) = block(below: 5pt)[
  #text(weight: 600)[#group:] #text(fill: rgb("#3a4656"))[#items]
]

// The two short bottom sections side by side.
#let resume-cols(left, right) = grid(columns: (1fr, 1fr), column-gutter: 26pt, left, right)

// The grey separator of the contact line.
#let resume-sep = text(fill: rgb("#9aa4b1"))[ · ]
