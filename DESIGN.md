# Design System — Hermes Dark

## 1. Visual Theme & Atmosphere

Inspired by Nous Research's Hermes Agent — a dark, warm interface with black canvas, cream-tinted text, and teal-dark panels. The amber radial gradient overlay adds atmospheric warmth. The aesthetic is editorial and restrained: uppercase display labels, wide letter-spacing, and monospace for technical detail create a terminal-meets-publishing feel.

Color is used with restraint. The palette centers on warm cream `#ffe6cb` against deep black `#000000`, with dark teal panels `#041C1C` providing surface structure. Accent blue `#74ade8` appears only for interactive focus. Semantic colors (green, amber, red) carry meaning — never decoration.

**Key Characteristics:**
- Black-first design with warm cream foreground and teal-dark surfaces
- Single accent blue (`#74ade8`) for focus and interactive states only
- Amber radial gradient overlay for atmospheric warmth
- Uppercase labels with wide letter-spacing for hierarchy
- Flat surfaces with 1px borders — no shadows
- Content-dominant layout with minimal chrome
- Monospace for technical detail, sans-serif for body

## 2. Color Palette & Roles

### Primary Surfaces
- **Background** (`#000000`): App frame, outermost layer
- **Card** (`#041C1C`): Panels, sidebar, elevated surfaces
- **Editor** (`#021212`): Content canvas, input backgrounds
- **Secondary** (`#0a2a2a`): Buttons, chips, subtle surfaces

### Text
- **Foreground** (`#ffe6cb`): Primary text, headings — warm cream
- **Muted** (`rgba(255,230,203,0.6)`): Secondary labels, inactive elements
- **Placeholder** (`rgba(255,230,203,0.35)`): Input placeholders, disabled

### Borders
- **Standard** (`rgba(255,230,203,0.15)`): Panel boundaries, dividers
- **Focus** (`rgba(116,173,232,0.5)`): Keyboard focus ring

### Accent
- **Blue** (`#74ade8`): Interactive focus, links, active indicators

### Semantic
- **Success** (`#a1c181`): Green, health OK
- **Warning** (`#dec184`): Amber, partial health
- **Error** (`#d07277`): Red, broken health, destructive actions
- **Neutral** (`#878a98`): Gray, hook-only state

### Overlay
- **Amber gradient**: `radial-gradient(ellipse at 0% 0%, transparent 60%, rgba(255,189,56,0.35) 100%)` with `mix-blend-mode: lighten` at 12% opacity

## 3. Typography Rules

### Font Families
- **UI / Body**: Inter — all menus, labels, descriptions
- **Display**: Inter (bold, uppercase, tracked) — brand, section kickers, nav pills
- **Monospace**: System mono stack — paths, IDs, counts, timestamps, version numbers

### Hierarchy

| Role | Size | Weight | Tracking | Transform | Notes |
|------|------|--------|----------|-----------|-------|
| Brand | 15px | 700 | 0.06em | uppercase | App title |
| Nav Pill | 12px | 500 | 0.1em | uppercase | Tab navigation |
| Kicker | 10-11px | 500 | 0.14em | uppercase | Section labels |
| Body | 14px | 400 | 0 | none | Descriptions, content |
| Emphasized | 14px | 500 | 0 | none | Active items, key labels |
| Chip/Badge | 12px | 500 | 0 | none | Tags, counts |
| Mono detail | 11-12px | 400 | 0 | none | Timestamps, IDs, paths |

### Principles
- **Three-weight system**: 400 (body), 500 (UI labels), 600-700 (headings/display)
- **Uppercase for structure**: Kickers, nav pills, and agent labels use uppercase with wide tracking
- **Monospace for technical**: Paths, session IDs, model names, timestamps, version numbers
- **Cream on black**: The warm cream foreground avoids the sterile feel of pure white

## 4. Component Stylings

### CSS Component Classes

| Class | Purpose |
|-------|---------|
| `.app-shell` | Root container, black bg |
| `.amber-overlay` | Fixed amber gradient atmosphere |
| `.surface-panel` | Panel with border, card bg |
| `.surface-card` | Rounded card with border |
| `.pill-tab` / `.pill-tab-active` / `.pill-tab-idle` | Navigation tabs, uppercase tracked |
| `.zed-kicker` | Uppercase section labels |
| `.zed-chip` | Inline tags with mono font |
| `.zed-input` | Text input fields |
| `.zed-list-row` | List items |

### Buttons (via shadcn/ui Button)
- **Default**: border + secondary bg, hover accent
- **Outline**: transparent bg, border, muted text
- **Destructive**: red border at 30% opacity, red text, red bg at 10%

### Cards/Panels
- Background: `--card` (#041C1C)
- Border: 1px `--border` (cream 15%)
- Radius: 6px (standard), 4px (tight)

### Input Fields
- Background: `--input` (#0a2a2a)
- Border: 1px `--border`
- Focus: border → `--ring` (blue 50%)
- Placeholder: cream 35%

### Scrollbar
- Track: transparent
- Thumb: cream 20%, hover cream 40%

## 5. Layout Principles

### Spacing
- Base unit: 4px
- Standard padding: 12-16px horizontal, 8-12px vertical
- Gap between sections: 8-12px

### Grid
- Sessions: `grid-cols-[360px_minmax(0,1fr)]` — fixed left, flexible right
- System: `grid-cols-[minmax(0,1.2fr)_320px]` — main + sidebar
- Skills: single column, full width

### Density
- Information-dense, compact spacing
- 28-32px row heights
- No decorative whitespace — space is structural

## 6. Depth & Elevation

| Level | Treatment | Use |
|-------|-----------|-----|
| Canvas | `#000000` pure black | Background |
| Surface | `#041C1C` + border | Panels, cards |
| Raised | `#0a2a2a` | Buttons, inputs |
| Overlay | `#000000` + backdrop blur | Modals, dialogs |

No drop shadows. Elevation through background steps and border only.

## 7. Design Tokens (CSS Variables)

```
--background: #000000
--foreground: #ffe6cb
--card: #041C1C
--secondary: #0a2a2a
--accent: #0a2a2a
--muted-foreground: rgba(255, 230, 203, 0.6)
--border: rgba(255, 230, 203, 0.15)
--primary: #74ade8
--destructive: #d07277
--chart-2: #a1c181
--chart-3: #dec184
--chart-5: #878a98
--editor: #021212
--element-hover: rgba(255, 230, 203, 0.05)
--element-active: rgba(255, 230, 203, 0.1)
```

## 8. Agent Prompt Guide

### Quick Reference
- Background: `#000000`, Card: `#041C1C`, Editor: `#021212`
- Text: `#ffe6cb` (primary), `rgba(255,230,203,0.6)` (muted)
- Border: `rgba(255,230,203,0.15)`
- Accent: `#74ade8` (blue), focus ring: `rgba(116,173,232,0.5)`
- Semantic: `#a1c181` (green), `#dec184` (amber), `#d07277` (red)

### Example Prompts
- "Create a panel on `#041C1C` background with `rgba(255,230,203,0.15)` border. Text in `#ffe6cb` 14px. Selected row: `rgba(255,230,203,0.1)` background."
- "Build a kicker label: 10px, weight 500, uppercase, `tracking-[0.14em]`, `rgba(255,230,203,0.6)` color."
- "Design a nav pill: 12px, weight 500, uppercase, `tracking-[0.1em]`. Active: border + card bg. Idle: transparent, muted text."
