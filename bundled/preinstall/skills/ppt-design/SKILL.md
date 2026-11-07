---
name: "ppt-design"
description: "Improve presentation visuals: color schemes, typography, layout, chart choice and hierarchy. Use when polishing slides or asking for slide design advice."
description_zh: "优化演示文稿视觉：配色、字体、排版、图表选择与层次，适用于美化幻灯片和设计建议"
version: "1.0.0"
source: agentz
---

# PPT Visual Design

Professional visual design advice and optimization for presentation decks: color schemes, typography, layout, chart choice and visual hierarchy.

## When to Use

- The user mentions slide colors, fonts, layout or other visual topics
- The user asks to polish or beautify an existing deck
- The user asks about presentation design rules and best practices
- The user needs to pick a chart type
- The user wants visual style suggestions

## Core Capabilities

### 1. Color schemes

**Business**
```
Primary: navy #0A2463
Secondary: gold #D4AF37, light gray #E8E8E8
Accent: deep red #C1292E
Background: white #FFFFFF
```

**Technology**
```
Primary: tech blue #1E90FF
Secondary: dark gray #2C3E50, neon green #00FF7F
Accent: purple #8B00FF
Background: dark #1A1A1A or white #FFFFFF
```

**Education**
```
Primary: warm orange #FF6B35
Secondary: sky teal #4ECDC4, lemon yellow #FFE66D
Accent: dark blue #293241
Background: off-white #F7FFF7
```

**Minimal**
```
Primary: black #000000
Secondary: white #FFFFFF
Accent: a single brand color
Background: white or light gray
```

### 2. Font pairing

Heading fonts:
- **Montserrat**: modern, geometric, good for business
- **Roboto**: clear, neutral, good for technical topics
- **Poppins**: friendly, rounded, good for education
- **Helvetica**: classic, clean, good for minimal designs

Body fonts:
- **Open Sans**: readable, professional
- **Lato**: elegant, modern
- **Nunito**: friendly, approachable
- **Source Sans Pro**: technical, clear

Usage rules:
```
Title: 28-48 pt, bold
Subtitle: 20-28 pt, medium
Body: 18-24 pt, regular
Notes: 14-16 pt, light

Minimum size: 14 pt (must stay readable when projected)
Number of fonts: 2-3 at most
Contrast: text to background contrast ratio >= 4.5:1
```

### 3. Layout grid

**Title slide**
```
┌─────────────────────────────┐
│                             │
│         [Big title]         │
│         [Subtitle]          │
│                             │
│       [Author / date]       │
│                             │
└─────────────────────────────┘
```

**Single column**
```
┌─────────────────────────────┐
│ [Title]                     │
│ ─────────────────────────── │
│ • Point 1                   │
│ • Point 2                   │
│ • Point 3                   │
│                             │
│       [Chart / image]       │
└─────────────────────────────┘
```

**Two columns**
```
┌─────────────────────────────┐
│ [Title]                     │
│ ─────────────────────────── │
│ • Point 1    │  [Image]     │
│ • Point 2    │   or         │
│ • Point 3    │  [Chart]     │
│              │              │
└─────────────────────────────┘
```

**Chart slide**
```
┌─────────────────────────────┐
│ [Title]                     │
│ ─────────────────────────── │
│                             │
│     [Large chart / image]   │
│                             │
│   [Data notes or takeaways] │
└─────────────────────────────┘
```

White space rules:
- Margins: at least 10% (5% left/right, 5% top/bottom)
- Spacing: leave 20-30 px below the title
- Line spacing: 1.2-1.5x
- Never fill the whole slide

### 4. Choosing a chart type

- **Comparison**: column chart (categories), bar chart (many items), radar chart (multiple dimensions)
- **Trend**: line chart (change over time), area chart (magnitude of change), waterfall chart (increases and decreases)
- **Proportion**: pie chart (whole share, at most 5 categories), donut chart (highlight a share), treemap (hierarchical share)
- **Relationship**: scatter plot (correlation), bubble chart (three variables), heat map (matrix relations)
- **Process**: flowchart (steps and decisions), timeline (event order), swimlane diagram (cross-team processes)
- **Hierarchy**: org chart (reporting lines), pyramid (layers), mind map (knowledge structure)

### 5. Visual hierarchy

**Size contrast**
```
Title: 48 pt
Subtitle: 28 pt
Body: 20 pt
Notes: 14 pt
```

**Weight contrast**
```
Title: Bold (700)
Subtitle: SemiBold (600)
Body: Regular (400)
Notes: Light (300)
```

**Color contrast**
```
Primary information: dark (black / dark blue)
Secondary information: mid gray
Supporting information: light gray
Emphasis: brand or accent color
```

**Position**
```
Important information: upper area, center
Secondary information: lower area, edges
```

### 6. Icons and images

Icons:
- Keep one consistent style from a single icon set
- Same size for the same level of hierarchy
- Colors match the palette
- Meaning must be unambiguous

Recommended icon sets: Lucide Icons (open source, modern), Heroicons (clean), Font Awesome (rich, classic), Material Icons (standard).

Images:
- High quality, resolution >= 1920x1080
- Directly relevant to the content
- Consistent style and tone
- Licensed or free for commercial use

Image sources: Unsplash, Pexels, Pixabay.

## Design Checklist

Overall style
- [ ] No more than 3 main colors
- [ ] No more than 2-3 fonts
- [ ] Consistent visual style
- [ ] Brand elements applied correctly

Readability
- [ ] Minimum font size >= 14 pt
- [ ] Sufficient text/background contrast
- [ ] Moderate amount of text per slide (at most 5 points)
- [ ] Comfortable line spacing (1.2-1.5x)

Layout and typography
- [ ] Enough white space
- [ ] Elements aligned
- [ ] Clear visual hierarchy
- [ ] Reasonable information density

Charts and images
- [ ] Suitable chart type
- [ ] Clear data labels
- [ ] High-resolution images
- [ ] Consistent icon style

Professionalism
- [ ] No spelling or grammar errors
- [ ] Accurate, credible data
- [ ] Sources cited
- [ ] Complete contact details

## Feedback Template

When reviewing an existing deck, give suggestions in this structure:

```markdown
# Deck Design Review

## 1. Color
**Current issue**: [describe]
**Proposal**: [new color scheme]
**Reason**: [why]

## 2. Typography
**Current issue**: [describe]
**Proposal**: [recommended font pairing]
**Reason**: [why]

## 3. Layout
**Current issue**: [describe]
**Proposal**: [new layout]
**Reason**: [layout principle]

## 4. Charts
**Current issue**: [describe]
**Proposal**: [chart type and style]
**Reason**: [data visualization principle]

## 5. Overall improvements
- [Improvement 1]
- [Improvement 2]
- [Improvement 3]
```

## Common Problems and Fixes

### Information overload
Symptom: too much text per slide.
Fix: distill the key message, cut redundancy, split into multiple slides with one point each, replace paragraphs with charts, add white space.

### Chaotic colors
Symptom: too many colors, no primary/secondary distinction.
Fix: pick 1-2 main colors, use tints and shades of the same hue, reserve the accent color for key content, start from a proven palette.

### Messy fonts
Symptom: many fonts mixed, inconsistent style.
Fix: limit to at most 3 fonts, one for headings and one for body, build hierarchy with size and weight, avoid decorative fonts.

### Crowded layout
Symptom: elements touch the edges, no breathing room.
Fix: set sensible margins, increase spacing between elements, reduce content per slide, align to a grid.

### Unclear charts
Symptom: complex charts that are hard to read.
Fix: simplify the data to the essentials, choose a more intuitive chart type, add clear labels and captions, use contrasting colors to highlight the point.

## Quick Tips

### Five-minute polish
1. Unify fonts (one heading font plus one body font)
2. Apply a color scheme (one primary plus one secondary)
3. Align all elements (use alignment tools)
4. Add white space (adjust margins and spacing)
5. Unify icon style (replace with a single icon set)

### Quick color picks
- Blue: professional, trustworthy, technical
- Green: health, environment, growth
- Orange: energetic, creative, warm
- Purple: premium, innovative, mysterious
- Gray: simple, modern, neutral

### When time is short
1. Solid background with white text (or the reverse)
2. A single font, with hierarchy from size alone
3. Simple column or line charts
4. Minimal decoration, focus on content
5. Make sure contrast and readability hold up

## Resources

Inspiration: Behance, Dribbble, SlideShare, Pinterest.
Tools: Canva (online design), Figma (prototyping), Coolors (palette generator), ColorHunt (palette inspiration).
