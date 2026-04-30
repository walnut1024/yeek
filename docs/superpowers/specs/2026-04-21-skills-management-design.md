# Skills Management Feature — Design Spec

## Goal

Add a top-level "Skills" tab to Yeek that lets users view, toggle, and uninstall all Claude Code skills and plugins installed on their computer — global and project-level — with health validation that matches what Claude Code's `/plugins` command sees.

## Background

Claude Code skills come from three sources:

1. **Plugin-bundled**: The dominant source. Plugins are installed from marketplaces into `~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/`. Each plugin may contain a `skills/` directory with `SKILL.md` files and an `agents/` directory with `.md` agent definitions.
2. **Standalone global**: User-created skills in `~/.claude/skills/<name>/SKILL.md`.
3. **Project-level**: Skills in `<project_path>/.claude/skills/` and agents in `<project_path>/.claude/agents/`.

The plugin registry is at `~/.claude/plugins/installed_plugins.json`. Enable/disable state is in `~/.claude/settings.json` under `enabledPlugins` with keys in `<plugin>@<marketplace>` format. Marketplace metadata is in `~/.claude/plugins/known_marketplaces.json`.

**Key problem**: The registry can be stale — entries may point to paths that don't exist or lack manifests. Health validation is essential to surface broken plugins.

## Data Sources

| Source | Path | Scope |
|--------|------|-------|
| Plugin registry | `~/.claude/plugins/installed_plugins.json` | Global |
| Enable/disable state | `~/.claude/settings.json` → `enabledPlugins` | Global |
| Plugin disk cache | `~/.claude/plugins/cache/<market>/<plugin>/<ver>/` | Global |
| Marketplace registry | `~/.claude/plugins/known_marketplaces.json` | Global |
| Standalone skills | `~/.claude/skills/<name>/SKILL.md` | Global |
| Project skills | `<project_path>/.claude/skills/<name>/SKILL.md` | Project |
| Project agents | `<project_path>/.claude/agents/<name>.md` | Project |
| Known project paths | `SELECT DISTINCT project_path FROM sessions` | Project |

## Health Validation

For each plugin, verify:
1. `installPath` directory exists on disk
2. `.claude-plugin/plugin.json` manifest exists
3. `skills/` subdirectories contain `SKILL.md` files
4. `agents/` directory contains `.md` files

Health states:

| State | Color | Criteria |
|-------|-------|----------|
| OK | Green | Manifest found, skills/agents loaded |
| PARTIAL | Amber | No manifest, but skill/agent files exist on disk |
| HOOK | Gray | Path exists but no skills and no agents (hook-only plugin) |
| BROKEN | Red | Path missing or empty despite registry entry |

For individual skills: check that `SKILL.md` exists and has valid YAML frontmatter (`---` delimited, contains `name` and `description`).

## Backend (Rust)

### New Types

```rust
struct PluginInfo {
    key: String,           // "superpowers@claude-plugins-official"
    name: String,
    version: String,
    scope: String,         // "global" | "project"
    marketplace: Option<MarketplaceInfo>,
    install_path: String,
    enabled: bool,
    health: String,        // "ok" | "partial" | "hook" | "broken"
    health_issues: Vec<String>,
    skills: Vec<SkillInfo>,
    agents: Vec<SkillInfo>,
    installed_at: Option<String>,
    last_updated: Option<String>,
}

struct SkillInfo {
    name: String,
    description: String,
    skill_type: String,    // "skill" | "agent"
    tools: Option<String>,
    file_path: String,
    health: String,        // "ok" | "warn" | "missing"
    health_detail: Option<String>,
}

struct MarketplaceInfo {
    name: String,
    repo: String,
    last_updated: Option<String>,
}

struct SkillsOverview {
    plugins: Vec<PluginInfo>,
    total_plugins: usize,
    total_skills: usize,
    total_agents: usize,
    health_summary: HealthSummary,
}

struct HealthSummary {
    ok: usize,
    partial: usize,
    hook: usize,
    broken: usize,
}
```

### New Tauri Commands

**`list_plugins(scope: String) -> Result<SkillsOverview, AppError>`**

When `scope == "global"`:
1. Read `~/.claude/plugins/installed_plugins.json` → map of plugin keys to install entries
2. Read `~/.claude/settings.json` → `enabledPlugins` map (default: enabled if absent)
3. Read `~/.claude/plugins/known_marketplaces.json` → marketplace metadata
4. For each plugin entry:
   a. Check if `installPath` exists on disk
   b. Check for `.claude-plugin/plugin.json`
   c. Scan `skills/` for `SKILL.md` files, parse YAML frontmatter
   d. Scan `agents/` for `.md` files, parse frontmatter
   e. Determine health status
   f. Attach marketplace info (name, repo, last_updated)
5. Scan `~/.claude/skills/` for standalone skills
6. Compute totals and health summary
7. Return `SkillsOverview`

When `scope == "project"`:
1. Query `SELECT DISTINCT project_path FROM sessions` from SQLite
2. For each project path:
   a. Check if `<path>/.claude/skills/` exists, scan for `SKILL.md`
   b. Check if `<path>/.claude/agents/` exists, scan for `.md`
   c. Build `PluginInfo` entries (one per project that has skills/agents)
3. Return `SkillsOverview`

**`toggle_plugin(key: String) -> Result<(), AppError>`**

1. Read `~/.claude/settings.json`
2. Toggle `enabledPlugins[key]`: if `true` → `false`, if absent/false → `true`
3. Write back `settings.json`

**`uninstall_plugin(key: String) -> Result<(), AppError>`**

1. Read `installed_plugins.json`, find entry by key, get `installPath`
2. Remove the `installPath` directory recursively (`std::fs::remove_dir_all`)
3. Remove entry from `installed_plugins.json`, write back
4. Remove entry from `enabledPlugins` in `settings.json` if present, write back

### YAML Frontmatter Parsing

`SKILL.md` and agent `.md` files use `---` delimited YAML frontmatter:

```yaml
---
name: brainstorming
description: Help turn ideas into fully formed designs...
tools: Read, Glob, Grep, Bash
model: opus
---
```

Parse with a lightweight YAML parser (or simple string extraction — only need `name`, `description`, `tools`, `model`). Use `serde_yaml` or manual extraction.

## Frontend

### UI Structure

**Header**: Add "Skills" tab between Sessions and System.

**Skills Page** toolbar:
- Global/Project toggle button (two-state)
- Plugins / All Skills view toggle
- Count chip (total skills + agents)

**Health summary bar**: OK / Partial / Hook / Broken counts with colored dots.

**Plugin view** (default):
- Expandable plugin cards, each showing:
  - Name, version
  - Marketplace name + repo (inline, e.g. `← claude-plugins-official anthropics/claude-plugins-official`)
  - Health badge (colored)
  - Enable/disable toggle switch
  - Uninstall button
- Expanded detail:
  - Install path (monospace, copy button)
  - Marketplace row (name + repo + last updated)
  - Health detail (what was checked, what failed)
  - Skill/agent list with individual health dots
- Standalone section for `~/.claude/skills/`

**Project view** (toggled):
- Same card layout but shows project-level skills/agents grouped by project path
- Empty state: "No project-level skills found"

**Flat view** (toggled):
- All skills/agents across all plugins in a flat list
- Each row: type badge (Skill/Agent), name, description, source (`plugin ← marketplace`), health dot

**Uninstall dialog**: AlertDialog with plugin name + install path + confirmation.

### New Files

| File | Purpose |
|------|---------|
| `src/pages/skills/skills-page.tsx` | Main Skills page component |

### Modified Files

| File | Change |
|------|--------|
| `src/app/shell/index.tsx` | Add "Skills" tab to header nav, render SkillsPage |
| `src/lib/api.ts` | Add `listPlugins`, `togglePlugin`, `uninstallPlugin` wrappers |
| `src/i18n/locales/en.json` | ~25 new keys for skills UI |
| `src/i18n/locales/zh-CN.json` | ~25 new keys for skills UI |

### i18n Keys (new)

```
skills.title, skills.kicker, skills.countChip,
skills.global, skills.project,
skills.viewPlugins, skills.viewAllSkills,
skills.health, skills.healthOk, skills.healthPartial, skills.healthHook, skills.healthBroken,
skills.pluginsSection, skills.standaloneSection, skills.projectSection,
skills.path, skills.market, skills.marketUpdated,
skills.uninstall, skills.uninstallTitle, skills.uninstallDesc, skills.uninstallPath,
skills.emptyProject, skills.emptyStandalone,
skills.skillType, skills.agentType,
skills.tools, skills.moreSkills, skills.disabled
```

## Mockup

Visual mockup saved at `ui_design/skills-page.html` — dark theme matching DESIGN.md spec with Zed-inspired palette.

## Out of Scope

- Installing new plugins/skills from marketplaces (future)
- Editing skill content
- Creating new skills
- Plugin update checking
