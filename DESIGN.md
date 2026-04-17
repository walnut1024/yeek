# Design System Inspired by Zed Editor

## 1. Visual Theme & Atmosphere

Zed's editor UI is defined by restraint, density, and the philosophy that "your editor should disappear." Every pixel carries its weight — the UI chrome recedes entirely, letting content (code, file trees, terminal output) dominate the frame. There is no decoration for its own sake. The result is a product that feels like a precision instrument: engineered, not designed.

The default theme One Dark adopts a **cool blue-gray palette** — backgrounds built on `#282c33` (editor), `#2f343e` (panels), `#3b414d` (app frame) — with text climbing through muted blue-grays toward off-white `#dce0e5`. This cool temperature gives the interface a calm, focused feel without the sterility of pure neutral grays. One Light inverts the scheme with near-white `#fafafa` editor and `#dcdcdd` frame, while retaining the same structural logic.

Color is used sparingly and deliberately. The **accent blue** (`#74ade8` in dark, `#5c78e2` in light) is the single saturated accent, reserved for focus borders, active indicators, cursor color, and link text. Semantic colors — red `#d07277` for errors, green `#a1c181` for success, amber `#dec184` for warnings — appear only where they carry meaning: diagnostics, version control status, lint markers. There is no decorative gradient, no rainbow of accents, no visual noise.

What makes Zed distinctive is its GPU-native rendering via the custom GPUI framework: all UI elements (quads, text sprites, underlines, selections) are compiled into a Scene and drawn in one GPU pass at 120FPS. This performance-first architecture directly shapes the visual language — no drop shadows (expensive), no blurs, no gradients — just flat color steps and 1px borders.

**Key Characteristics:**
- Dark-first design with cool blue-gray neutrals (One Dark default)
- Single accent color (`#74ade8` dark / `#5c78e2` light) used with surgical precision
- Content-dominant layout: UI chrome minimized, code and data fill the frame
- Dense information architecture: compact spacing, small type, high information density
- Flat surfaces with no gradients, no shadows for decoration
- 1px solid borders as the primary visual separator (not background shifts or shadows)
- Monochrome iconography; color only for semantic state
- GPU-native rendering at 120FPS — performance as visual identity
- 8-player collaboration color system for multi-user cursors and selections

## 2. Color Palette & Roles

### Accent Blue (One Dark)
- **Accent Text** (`#74ade8`): `text.accent`, links, active item labels, accent icons
- **Accent Hover** (`#3b9eff`): Hovered accent elements (derived from blue scale)
- **Focus Border** (`#47679e`): `border.focused`, keyboard focus rings
- **Selected Border** (`#293b5b`): `border.selected`, selected item outlines
- **Accent Highlight** (`#74ade81a`): 10% opacity tint for document read highlights

### Accent Blue (One Light)
- **Accent Text** (`#5c78e2`): `text.accent`, links, active labels
- **Focus Border** (`#7d82e8`): `border.focused`, focus rings (purple-blue shift)
- **Selected Border** (`#cbcdf6`): `border.selected`, selection outlines

### Text Colors (One Dark)
- **Primary** (`#dce0e5`): `text`, main body text, headings, default icons
- **Muted** (`#a9afbc`): `text.muted`, secondary labels, inactive tabs, dimmed icons
- **Placeholder** (`#878a98`): `text.placeholder`, input placeholders, disabled text
- **Editor Foreground** (`#acb2be`): `editor.foreground`, code default color (slightly dimmer than `text`)
- **Line Number** (`#4e5a5f`): `editor.line_number`, gutter line numbers
- **Active Line Number** (`#d0d4da`): `editor.active_line_number`, current line number

### Text Colors (One Light)
- **Primary** (`#242529`): `text`, near-black on white
- **Muted** (`#58585a`): `text.muted`, secondary labels
- **Placeholder** (`#7e8086`): `text.placeholder`, inputs, disabled items

### Surface & Background (One Dark)
- **App Frame** (`#3b414d`): `background`, title bar, status bar — the outermost layer
- **Surface** (`#2f343e`): `surface.background`, panels, sidebar, tab bar, elevated surfaces
- **Editor** (`#282c33`): `editor.background`, toolbar, editor gutter — the content canvas
- **Element Default** (`#2e343e`): `element.background`, buttons, inputs at rest
- **Element Hover** (`#363c46`): `element.hover`, ghost element hover
- **Element Active** (`#454a56`): `element.active`, pressed/selected state
- **Ghost Default** (`transparent`): `ghost_element.background`, toolbar buttons at rest
- **Active Line** (`#2f343ebf`): `editor.active_line.background`, 75% opacity current line
- **Drop Target** (`#83899480`): `drop_target.background`, drag-and-drop indicator

### Surface & Background (One Light)
- **App Frame** (`#dcdcdd`): `background`, title bar, status bar
- **Surface** (`#ebebec`): `surface.background`, panels, tab bar
- **Editor** (`#fafafa`): `editor.background`, toolbar
- **Element Default** (`#ebebec`): `element.background`
- **Element Hover** (`#dfdfe0`): `element.hover`
- **Element Active** (`#cacaca`): `element.active`

### Borders (One Dark)
- **Standard** (`#464b57`): `border`, panel boundaries, dividers, component outlines
- **Variant** (`#363c46`): `border.variant`, lighter/subtle borders
- **Focused** (`#47679e`): `border.focused`, keyboard focus ring (blue-tinted)
- **Selected** (`#293b5b`): `border.selected`, selection outlines
- **Disabled** (`#414754`): `border.disabled`, inactive element borders
- **Transparent** (`transparent`): `border.transparent`, placeholder for layout alignment

### Semantic
- **Error** (`#d07277`): Diagnostics failures, with bg `#d072771a` and border `#4c2b2c`
- **Warning** (`#dec184`): Lint warnings, with bg `#dec1841a` and border `#5d4c2f`
- **Info** (`#74ade8`): Informational hints, with bg `#74ade81a` and border `#293b5b`
- **Hint** (`#788ca6`): Subtle suggestions, with bg `#5a6f891a` and border `#293b5b`
- **Success** (`#a1c181`): Operation success, with bg `#a1c1811a` and border `#38482f`

### Version Control
- **Added** (`#27a657` VCS / `#a1c181` marker): Git additions, green
- **Modified** (`#d3b020` VCS / `#dec184` marker): Git modifications, amber
- **Deleted** (`#e06c76` VCS / `#d07277` marker): Git deletions, red
- **Conflict** (`#dec184`): Merge conflicts, amber
- **Renamed** (`#74ade8`): Renamed files, blue

### Shadows
- Zed uses **no decorative shadows**. There is no shadow scale, no `box-shadow`, no glow.
- Depth is communicated exclusively through background color steps and 1px solid borders.
- Where other editors use `box-shadow: 0px 0px 0px 1px ...`, Zed uses flat `border: 1px solid`.

## 3. Typography Rules

### Font Families
- **UI**: System UI font stack or `Inter` — all menus, labels, status bar, panel headers
- **Editor**: User-configurable, default `Zed Plex Mono`, common choices `JetBrains Mono`, `Menlo`, `Monaco`, `Fira Code`
- **Terminal**: Same as editor monospace — used for ANSI output, command results

### Hierarchy

| Role | Font | Size | Weight | Line Height | Notes |
|------|------|------|--------|-------------|-------|
| Dialog Title | Inter | 24–28px (1.50–1.75rem) | 600 | 1.3 (tight) | Settings pages, dialog headers |
| Panel Header | Inter | 16–18px (1.00–1.13rem) | 600 | 1.4 | Panel titles, section labels |
| Body | Inter | 13–14px (0.81–0.88rem) | 400 | 1.5 | Descriptions, panel content |
| Body Emphasized | Inter | 13–14px (0.81–0.88rem) | 500 | 1.5 | Active items, key labels |
| Tab / Badge | Inter | 11–12px (0.69–0.75rem) | 500 | 1.4 | File tabs, status badges |
| Micro | Inter | 10–11px (0.63–0.69rem) | 400 | 1.3 (tight) | Line numbers, timestamps |
| Code Body | Zed Plex Mono | 16px (1.00rem) | 400 | 1.6 (relaxed) | Editor code, default `buffer_font_size` |
| Code UI | Zed Plex Mono | 13px (0.81rem) | 400 | 1.6 (relaxed) | Inline code, command palette items |

### Principles
- **Small is correct**: Zed deliberately uses 13px for UI body text. The dense layout prioritizes information over comfort. Do not increase type sizes to "improve readability" — density is the design intent.
- **Three-weight system**: 400 (regular) is default body. 500 (medium) for interactive labels and active items. 600 (semibold) for headings only. Never use 700+ in UI (the sole 700 exception: `emphasis.strong` in syntax highlighting).
- **Monospace as first-class citizen**: File paths, session IDs, tool names, command output, version numbers, PR references — all monospace. The code font is not a fallback; it is an equal member of the type system.
- **No letter-spacing adjustment**: Let the font's natural spacing do the work. Zero tracking at all sizes, all weights.
- **Uniform 1.4–1.5 line-height**: Consistent moderate leading across body text. Not tight, not loose. Tighter (1.3) only for micro text and dialog titles.

## 4. Component Stylings

### Buttons

**Ghost Button (Default)**
- Background: `transparent`
- Hover: `#363c46` (`element.hover`)
- Active: `#454a56` (`element.active`)
- Text: `#dce0e5` (`text`)
- Border: none (or `transparent` placeholder)
- Padding: 4px 8px
- Radius: 6px
- Use: Toolbar actions, sidebar buttons, most interactive elements

**Filled Button**
- Background: `#2e343e` (`element.background`)
- Hover: `#363c46` (`element.hover`)
- Text: `#dce0e5` (`text`)
- Border: 1px solid `#464b57` (`border`)
- Padding: 6px 12px
- Radius: 6px
- Use: Dialog confirmations, primary actions

**Accent Button**
- Background: `#74ade8` (`text.accent`)
- Hover: lighter variant
- Text: `#ffffff` or dark contrast
- Padding: 6px 12px
- Radius: 6px
- Use: Key CTAs only (install extension, destructive confirmations) — use sparingly

### Tab Bar
- Bar background: `#2f343e` (`tab_bar.background`)
- Inactive tab: `#2f343e` bg, `#a9afbc` text (`text.muted`)
- Active tab: `#282c33` bg (matches editor), `#dce0e5` text (`text`)
- Active tab merges visually with the editor below — no bottom border needed
- Close button: appears on hover only, `#a9afbc` icon, no background
- Modified indicator: small dot beside filename

### Panels & Sidebar
- Background: `#2f343e` (`panel.background`)
- Border: 1px solid `#464b57` (`border`) on dividing edge
- Active item: `#454a56` bg (`element.selected`)
- Focused panel border: `null` in One Dark (no visible focus border)
- Indent guides: `panel.indent_guide` / `panel.indent_guide_active`
- Tree structure: ~12px indent per nesting level
- Project Panel toggle: `cmd-b` (visibility), `cmd-shift-e` (focus)

### Editor Area
- Background: `#282c33` (`editor.background`)
- Default text: `#acb2be` (`editor.foreground`)
- Line numbers: `#4e5a5f` (dim), current `#d0d4da` (bright)
- Current line: `#2f343ebf` (75% opacity surface color)
- Cursor: `#74ade8` (Player 1 accent)
- Selection: `#74ade83d` (24% opacity accent)
- Wrap guide: `#c8ccd40d` (5% opacity)
- Invisible characters: `#4e5a5f` (same as line numbers)
- Minimap: 80–100px right gutter

### Input Fields
- Background: `#2e343e` (`element.background`)
- Border: 1px solid `#464b57` (`border`)
- Focus: border changes to `#47679e` (`border.focused`)
- Placeholder: `#878a98` (`text.placeholder`)
- Padding: 4px 8px
- Radius: 6px

### Context Menu / Dropdown
- Background: `#2f343e` (`elevated_surface.background`)
- Hover item: `#363c46` (`element.hover`)
- Separator: 1px solid `#464b57` (`border`)
- Shortcut text: `#878a98` (`text.placeholder`), right-aligned
- Border: 1px solid `#464b57`
- Radius: 8px
- Shadow: none (flat, border-defined)

### Scrollbar
- Track: `transparent`, border `#2e333c`
- Thumb: `#c8ccd44c` (~30% opacity)
- Thumb hover: `#363c46`
- Width: ~8px, auto-hide when not scrolling

### Status Bar
- Background: `#3b414d` (`status_bar.background`, matches app frame)
- Text: `#a9afbc` (`text.muted`)
- Icons: `#a9afbc` default, semantic colors for VCS indicators
- Border-top: 1px solid `#464b57`
- Content: language mode, line:column, error/warning counts, VCS branch

### Tooltip
- Background: `#454a56` (`element.active`)
- Text: `#dce0e5` (`text`)
- Padding: 4px 8px
- Radius: 4px
- No border, no shadow

### Command Palette
- Modal overlay with semi-transparent backdrop
- Input: `editor.background` + `border.focused` ring
- List items: 24–28px height, hover `element.hover`
- Shortcut hints: right-aligned, `text.placeholder` color
- Fuzzy match highlight: `text.accent` color
- Radius: 8–12px (dialog level)

### Terminal
- Background: `#282c34` (`terminal.background`)
- Foreground: `#abb2bf` (`terminal.foreground`)
- Bright foreground: `#dce0e5`, dim foreground: `#636d83`
- ANSI palette: red `#e06c75`, green `#98c379`, yellow `#e5c07b`, blue `#61afef`, magenta `#c678dd`, cyan `#56b6c2`
- Opens in any buffer tab position (`ctrl-`\``)
- Backend: Alacritty renderer

## 5. Layout Principles

### Spacing System
- Base unit: 4px
- Scale: 2px, 4px, 6px, 8px, 12px, 16px, 20px, 24px, 32px
- Horizontal padding for UI elements: 8px–12px
- Vertical padding for UI elements: 4px–6px
- Gap between sections: 12px–16px (not 24–32px — density over whitespace)

### Grid & Container
- No max-width container — editor fills entire viewport
- Sidebar: fixed width 240–280px, user-resizable by drag
- Bottom panel: fixed height, user-resizable by drag
- Center (editor): flexible, fills remaining space
- Minimap: 80–100px right gutter
- Pane splits: `cmd-k` + direction key, double-click divider to equalize

### Window Regions
| Region | Background | Position |
|--------|-----------|----------|
| Title Bar | `#3b414d` (app frame) | Top |
| Tab Bar | `#2f343e` (surface) | Below title bar |
| Toolbar | `#282c33` (editor) | Below tab bar |
| Sidebar | `#2f343e` (surface) | Left (or right) |
| Editor | `#282c33` (editor) | Center, fills remaining |
| Bottom Panel | `#2f343e` (surface) | Bottom, resizable |
| Status Bar | `#3b414d` (app frame) | Very bottom |

### Density Philosophy
- **Information-dense by default**: Panels pack controls tightly. Lists use 24–28px row heights. Whitespace is structural, not decorative — it separates groups, not individual items.
- **Compact is correct**: Do not add padding "for breathing room." Sidebar items sit at 24px row height with 4px horizontal padding and 12px indent per nesting level.
- **Content dominates**: The editor area is maximized. UI chrome (tab bar, status bar, sidebar) occupies the minimum viable footprint.

### Border Radius Scale
- Minimal (4px): Tooltips, micro elements
- Standard (6px): Buttons, inputs, tags, small containers (the workhorse radius)
- Comfortable (8px): Context menus, dropdowns, panels
- Generous (12px): Modals, dialogs, command palette
- Full (9999px): Not used — Zed avoids pill shapes entirely

## 6. Depth & Elevation

| Level | Treatment | Use |
|-------|-----------|-----|
| Flat (Level 0) | Same as parent bg | Editor content, empty panels |
| Surface (Level 1) | `#2f343e` surface bg | Sidebar, tab bar, bottom panel |
| Frame (Level 2) | `#3b414d` app frame bg | Title bar, status bar (outermost) |
| Elevated (Level 3) | `#2f343e` bg + 1px `#464b57` border | Context menus, popovers, dropdowns |
| Modal (Level 4) | `#2f343e` bg + 1px `#464b57` border + backdrop | Command palette, dialogs |

**Elevation Philosophy**: Zed does not use drop shadows. Elevation is expressed through background color steps (darker = further back, lighter = closer to user) and border presence. The hierarchy in One Dark reads: editor `#282c33` (deepest) → panels `#2f343e` (mid) → app frame `#3b414d` (outermost/brightest). A context menu "floats" because its `#2f343e` background sits on the `#282c33` editor canvas with a visible `#464b57` border outlining its shape — not because it casts a shadow.

## 7. Do's and Don'ts

### Do
- Use cool blue-gray neutrals for all UI chrome — never warm brown-gray or pure neutral
- Keep the accent blue (`#74ade8`) for interactive focus states only — not decoration
- Use 1px solid borders for separation — not shadows or background color alone
- Pack information densely — 28px list rows, 13px UI body text, 8px gaps
- Use monospace for anything technical: paths, IDs, commands, output, version numbers
- Let code content dominate the frame — minimize UI chrome to borders and subtle backgrounds
- Use ghost buttons (transparent bg, visible on hover) as the default button style
- Reserve filled/accent buttons for the single most important action per context
- Match active tab background to the editor canvas to create visual continuity
- Use the three-layer background system: editor → surface → app frame

### Don't
- Don't use drop shadows anywhere — elevation is border + background step
- Don't use gradients — flat colors only
- Don't add colored backgrounds to content areas — neutrals carry the structure
- Don't use rounded pill shapes (9999px radius) — 6px–8px is the maximum
- Don't use the accent blue for large filled areas — it's an accent, not a primary fill
- Don't increase font sizes or spacing beyond spec — the density is intentional, not a bug
- Don't use weight 700+ in UI — 600 is the ceiling
- Don't add illustrations or decorative imagery to the editor UI — content is the visual
- Don't use semantic colors (red/green/amber) for decoration — they carry diagnostic meaning
- Don't use icons with color fills — monochrome only, matching `icon` / `icon.muted` color

## 8. Responsive Behavior

### Window Regions
| Region | Minimum | Default | Resizable |
|--------|---------|---------|-----------|
| Sidebar | Collapsible (0px) | 240–280px | Yes, drag edge |
| Bottom Panel | Collapsible (0px) | ~200px | Yes, drag edge |
| Editor | Fills remaining | Fills remaining | Automatic |
| Tab Bar | Fixed height ~32px | ~32px | No |
| Status Bar | Fixed height ~24px | ~24px | No |

### Collapsing Strategy
- Sidebar: `cmd-b` toggles visibility; collapses to 0px, no residual chrome
- Bottom panel: `shift-escape` toggles; anchors to right, bottom, or modal mode (Dock)
- Pane splits: `cmd-k` + direction to split; double-click divider to equalize; panes can be closed individually
- Tab bar: scrollable when tabs exceed width; no wrapping or stacking
- Minimap: hides automatically when editor width is narrow
- Window narrow state: sidebar collapses first, then bottom panel, editor always fills remaining

## 9. Agent Prompt Guide

### Quick Color Reference
- Editor: `#282c33` (canvas), `#2f343e` (panels), `#3b414d` (app frame)
- Text: `#dce0e5` (primary), `#a9afbc` (muted), `#878a98` (placeholder)
- Code: `#acb2be` (editor foreground), `#4e5a5f` (line numbers), `#d0d4da` (active line number)
- Border: `#464b57` (standard), `#363c46` (variant), `#47679e` (focused)
- Accent: `#74ade8` (blue), `#3b9eff` (hover), `#293b5b` (selected border)
- Semantic: `#d07277` (error), `#dec184` (warning), `#a1c181` (success), `#74ade8` (info)
- VCS: `#27a657` (added), `#d3b020` (modified), `#e06c76` (deleted)
- Players: `#74ade8`, `#be5046`, `#bf956a`, `#b477cf`, `#6eb4bf`, `#d07277`, `#dec184`, `#a1c181`

### Example Component Prompts
- "Create a file list panel on `#2f343e` background, 1px solid `#464b57` right border. Each row 28px tall with 8px horizontal padding. Text `#dce0e5` 13px weight 400. Selected row: `#454a56` background. Hover row: `#363c46` background."
- "Design a tab bar: `#2f343e` background. Inactive tabs `#a9afbc` text on `#2f343e`. Active tab `#282c33` background (merges with editor), `#dce0e5` text. No bottom highlight — background match is the active indicator."
- "Build a context menu: `#2f343e` background, 1px solid `#464b57` border, 8px radius. Items 24px height, `#dce0e5` 13px text, hover `#363c46`. Separator 1px solid `#464b57`. Shortcuts `#878a98` right-aligned. No shadow."
- "Create an input field: `#2e343e` background, 1px solid `#464b57` border, 6px radius, 4px 8px padding. Placeholder `#878a98` 13px. Focus: border changes to `#47679e`."
- "Build a ghost button: `transparent` background, `#dce0e5` 13px weight 500 text, 6px radius, 4px 8px padding. Hover: `#363c46` background. Active: `#454a56`. No border in default state."

### Iteration Guide
1. Start with `#282c33` — the editor canvas. Everything is built around it.
2. Add `#2f343e` panels and sidebars with `#464b57` borders for structure
3. Wrap it all in `#3b414d` app frame (title bar + status bar)
4. Ghost buttons as default — transparent bg that appears on hover as `#363c46`
5. Text hierarchy: `#dce0e5` > `#a9afbc` > `#878a98` — three levels, no more
6. Reserve `#74ade8` / `#47679e` blue for focus rings and accent only
7. 13px UI text, 6px radius, 4px base unit — density over comfort, always
