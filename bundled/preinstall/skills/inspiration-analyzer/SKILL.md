---
name: "inspiration-analyzer"
description: "Analyze websites for design inspiration by extracting colors, typography, layouts, and patterns. Use when you have specific URLs to study for a design project."
description_zh: "分析网站的设计灵感：提取配色、字体、布局和设计模式。"
version: "1.0.0"
source: agentz
---

# Inspiration Analyzer

Analyze websites to extract design inspiration including colors, typography, layouts, and UI patterns.

## Purpose

When a user provides inspiration URLs, this skill:
- Visits each site with the `browser` tool
- Takes screenshots for visual analysis
- Extracts specific design elements
- Creates structured inspiration report
- Identifies replicable patterns

## Workflow

### Step 1: Open the Site

Use the `browser` tool to open the URL. If the `browser` tool is unavailable, fall back to `web_fetch` for the HTML/CSS source (see Fallback Mode).

### Step 2: Capture Screenshots

Take multiple screenshots to capture the full experience:

1. **Hero/Above-fold**: Initial viewport
2. **Scrolled sections**: Scroll down and capture each section
3. **Interactive states**: Hover on navigation, buttons
4. **Mobile view**: Resize the viewport to a mobile width (about 375x812)

Use `vision_context` (when available) to analyze the screenshots visually.

### Step 3: Read the Page Source

Use `web_fetch` or the `browser` tool to read the page HTML and CSS for color variables, font-family declarations, and breakpoints.

### Step 4: Analyze Elements

From screenshots and page content, extract:

#### Colors
- **Primary color**: Main brand color
- **Secondary colors**: Supporting palette
- **Background color**: Page and section backgrounds
- **Text colors**: Headings and body text
- **Accent colors**: CTAs, links, highlights

Note hex codes where visible.

#### Typography
- **Heading font**: Name if identifiable, or describe style
- **Body font**: Name or describe
- **Font weights**: Light, regular, bold usage
- **Size scale**: Relative sizes of elements
- **Line height**: Tight or generous
- **Letter spacing**: Tracking patterns

#### Layout
- **Grid system**: Column structure
- **White space**: Spacing philosophy
- **Section structure**: Full-width, contained, alternating
- **Navigation style**: Fixed, hidden, sidebar
- **Footer structure**: Minimal or comprehensive

#### UI Patterns
- **Buttons**: Shape, size, states
- **Cards**: Borders, shadows, corners
- **Icons**: Style (outlined, filled, custom)
- **Images**: Treatment, aspect ratios
- **Animations**: Motion patterns observed

### Step 5: Generate Report

Create a structured analysis:

```markdown
## Website Analysis: [URL]

### Screenshots
[Describe key screenshots taken]

### Color Palette
| Role | Hex | Usage |
|------|-----|-------|
| Primary | #xxx | [Where used] |
| Secondary | #xxx | [Where used] |
| Background | #xxx | [Where used] |
| Text | #xxx | [Where used] |
| Accent | #xxx | [Where used] |

### Typography
- **Headlines**: [Font name/description] - [weight]
- **Body**: [Font name/description] - [weight]
- **Scale**: [Size relationships]
- **Line height**: [Observation]

### Layout Patterns
- Grid: [Description]
- Spacing: [Description]
- Sections: [Description]

### UI Elements
- **Buttons**: [Description]
- **Cards**: [Description]
- **Navigation**: [Description]
- **Footer**: [Description]

### Key Takeaways
1. [What makes this design distinctive]
2. [Pattern worth replicating]
3. [Specific technique to use]

### What to Avoid
- [Any patterns from this site that are overused]
- [Elements that wouldn't translate well]
```

## Multiple Sites

When analyzing multiple URLs:
1. Analyze each separately
2. Create individual reports
3. Summarize common themes
4. Note contrasting approaches
5. Recommend which elements to combine

## Fallback Mode

If the `browser` tool is unavailable:

1. Inform user that live analysis requires browser access
2. Ask user to:
   - Share screenshots of the sites
   - Describe what they like about each
   - Paste any visible color codes
   - Note font names if visible
3. Work with provided information to create analysis

## Best Practices

### For Accurate Color Extraction
- Look for color variables in page inspection
- Check buttons for primary brand color
- Note background color on different sections
- Capture hover states for accent colors

### For Typography Identification
- Look for Google Fonts link in source
- Check font-family in computed styles
- Note relative sizes between h1, h2, body
- Observe tracking on headings vs body

### For Layout Analysis
- Resize viewport to see responsive behavior
- Note breakpoints where layout changes
- Count columns in grid layouts
- Measure (visually) spacing consistency

## Output

The analysis should provide:
1. Actionable color palette (hex codes)
2. Typography recommendations
3. Layout patterns to replicate
4. UI component inspiration
5. Clear direction for moodboard

See `references/extraction-techniques.md` for detailed extraction methods.