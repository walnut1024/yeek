# Skills Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Skills" tab to Yeek that lists all installed Claude Code plugins/skills, shows health status, and supports enable/disable toggling and uninstall.

**Architecture:** Backend reads Claude Code's JSON registry files + filesystem to build a plugin inventory with health checks. Frontend renders expandable plugin cards with marketplace info, health badges, toggle switches, and uninstall confirmation. Global vs Project scope toggles between `~/.claude/plugins/` and session-derived project paths.

**Tech Stack:** Rust (Tauri v2, serde_json, serde_yaml new dep), React + TypeScript + TanStack Query, Tailwind CSS, shadcn/ui AlertDialog

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src-tauri/src/domain/plugin.rs` | New: PluginInfo, SkillInfo, MarketplaceInfo, SkillsOverview, HealthSummary types |
| `src-tauri/src/domain/mod.rs` | Modify: add `pub mod plugin;` |
| `src-tauri/src/app/commands.rs` | Modify: add 3 commands (list_plugins, toggle_plugin, uninstall_plugin) |
| `src-tauri/src/lib.rs` | Modify: import + register 3 new commands |
| `src-tauri/Cargo.toml` | Modify: add `serde_yaml` dependency |
| `src/lib/api.ts` | Modify: add 3 Tauri invoke wrappers + types |
| `src/pages/skills/skills-page.tsx` | New: main Skills page UI |
| `src/app/shell/index.tsx` | Modify: add "skills" to section union, header nav, render |
| `src/i18n/locales/en.json` | Modify: add ~25 skills.* keys |
| `src/i18n/locales/zh-CN.json` | Modify: add ~25 skills.* keys |

---

### Task 1: Add domain types for plugin data

**Files:**
- Create: `src-tauri/src/domain/plugin.rs`
- Modify: `src-tauri/src/domain/mod.rs`

- [ ] **Step 1: Create `plugin.rs` with all domain types**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginInfo {
    pub key: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub marketplace: Option<MarketplaceInfo>,
    pub install_path: String,
    pub enabled: bool,
    pub health: String,
    pub health_issues: Vec<String>,
    pub skills: Vec<SkillInfo>,
    pub agents: Vec<SkillInfo>,
    pub installed_at: Option<String>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub skill_type: String,
    pub tools: Option<String>,
    pub file_path: String,
    pub health: String,
    pub health_detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketplaceInfo {
    pub name: String,
    pub repo: String,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillsOverview {
    pub plugins: Vec<PluginInfo>,
    pub total_plugins: usize,
    pub total_skills: usize,
    pub total_agents: usize,
    pub health_summary: HealthSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthSummary {
    pub ok: usize,
    pub partial: usize,
    pub hook: usize,
    pub broken: usize,
}
```

- [ ] **Step 2: Register module in `mod.rs`**

Add to `src-tauri/src/domain/mod.rs`:

```rust
pub mod plugin;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/plugin.rs src-tauri/src/domain/mod.rs
git commit -m "feat(skills): add domain types for plugin/skill data"
```

---

### Task 2: Add serde_yaml dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add serde_yaml to Cargo.toml**

Add under `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
serde_yaml = "0.9"
```

- [ ] **Step 2: Verify it builds**

Run: `cargo check`
Expected: resolves and compiles

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(skills): add serde_yaml dependency"
```

---

### Task 3: Implement backend — list_plugins command

**Files:**
- Modify: `src-tauri/src/app/commands.rs`
- Modify: `src-tauri/src/lib.rs`

This is the largest task. The command reads Claude Code's JSON files and filesystem to build a complete plugin inventory.

- [ ] **Step 1: Add `list_plugins` command to `commands.rs`**

Add this function at the end of `src-tauri/src/app/commands.rs`:

```rust
#[tauri::command]
pub fn list_plugins(
    state: State<'_, AppState>,
    scope: String,
) -> Result<plugin::SkillsOverview, AppError> {
    if scope == "project" {
        return list_project_plugins(&state);
    }
    list_global_plugins()
}

fn list_global_plugins() -> Result<plugin::SkillsOverview, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Read plugin registry
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let registry: serde_json::Value = read_json(&registry_path)?;

    // 2. Read enabled state
    let settings_path = claude_dir.join("settings.json");
    let settings: serde_json::Value = read_json(&settings_path)?;
    let enabled_map = settings.get("enabledPlugins").and_then(|v| v.as_object());

    // 3. Read marketplace metadata
    let marketplaces_path = claude_dir.join("plugins/known_marketplaces.json");
    let marketplaces: serde_json::Value = read_json_or_default(&marketplaces_path);

    let plugins_map = registry
        .get("plugins")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::ParseError("Invalid installed_plugins.json".into()))?;

    let mut plugins = Vec::new();
    let mut total_skills = 0usize;
    let mut total_agents = 0usize;
    let mut health_ok = 0usize;
    let mut health_partial = 0usize;
    let mut health_hook = 0usize;
    let mut health_broken = 0usize;

    for (key, entries) in plugins_map {
        let entries_arr = match entries.as_array() {
            Some(a) => a,
            None => continue,
        };
        let entry = match entries_arr.first() {
            Some(e) => e,
            None => continue,
        };

        let install_path = entry["installPath"].as_str().unwrap_or("").to_string();
        let version = entry["version"].as_str().unwrap_or("unknown").to_string();
        let installed_at = entry["installedAt"].as_str().map(String::from);
        let last_updated = entry["lastUpdated"].as_str().map(String::from);

        // Parse key: "plugin@marketplace"
        let parts: Vec<&str> = key.split('@').collect();
        let plugin_name = parts.first().map(|s| s.to_string()).unwrap_or_default();
        let market_name = parts.get(1).map(|s| s.to_string());

        // Enabled state
        let enabled = enabled_map
            .and_then(|m| m.get(key))
            .map(|v| v.as_bool().unwrap_or(true))
            .unwrap_or(true); // absent = enabled

        // Marketplace info
        let marketplace = market_name.as_ref().and_then(|mn| {
            let mkt = marketplaces.get(mn)?;
            let repo = mkt["source"]["repo"].as_str().unwrap_or("").to_string();
            let last_upd = mkt["lastUpdated"].as_str().map(String::from);
            Some(plugin::MarketplaceInfo {
                name: mn.clone(),
                repo,
                last_updated: last_upd,
            })
        });

        // Health check
        let path = std::path::Path::new(&install_path);
        let mut health_issues = Vec::new();

        let (skills, agents, health) = if !path.exists() {
            health_issues.push("Install path does not exist".into());
            (Vec::new(), Vec::new(), "broken")
        } else {
            let has_manifest = path.join(".claude-plugin/plugin.json").exists();
            let scanned_skills = scan_skills(path);
            let scanned_agents = scan_agents(path);

            if !has_manifest && scanned_skills.is_empty() && scanned_agents.is_empty() {
                if has_hooks(path) {
                    health_issues.push("Hook-only plugin, no skills or agents".into());
                    (scanned_skills, scanned_agents, "hook")
                } else {
                    health_issues.push("Missing plugin.json and no content".into());
                    (scanned_skills, scanned_agents, "broken")
                }
            } else if !has_manifest {
                health_issues.push("Missing plugin.json".into());
                (scanned_skills, scanned_agents, "partial")
            } else {
                (scanned_skills, scanned_agents, "ok")
            }
        };

        total_skills += skills.len();
        total_agents += agents.len();
        match health {
            "ok" => health_ok += 1,
            "partial" => health_partial += 1,
            "hook" => health_hook += 1,
            _ => health_broken += 1,
        }

        plugins.push(plugin::PluginInfo {
            key: key.clone(),
            name: plugin_name,
            version,
            scope: "global".into(),
            marketplace,
            install_path,
            enabled,
            health: health.into(),
            health_issues,
            skills,
            agents,
            installed_at,
            last_updated,
        });
    }

    Ok(plugin::SkillsOverview {
        total_plugins: plugins.len(),
        total_skills,
        total_agents,
        health_summary: plugin::HealthSummary {
            ok: health_ok,
            partial: health_partial,
            hook: health_hook,
            broken: health_broken,
        },
        plugins,
    })
}

fn list_project_plugins(state: &AppState) -> Result<plugin::SkillsOverview, AppError> {
    let db = state.db()?;
    let mut stmt = db.prepare("SELECT DISTINCT project_path FROM sessions WHERE project_path IS NOT NULL")
        .map_err(|e| AppError::DbError(e.to_string()))?;
    let paths: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| AppError::DbError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let mut plugins = Vec::new();
    let mut total_skills = 0usize;
    let mut total_agents = 0usize;

    for project_path in &paths {
        let path = std::path::Path::new(project_path);
        let skills_dir = path.join(".claude/skills");
        let agents_dir = path.join(".claude/agents");

        let skills = if skills_dir.exists() { scan_skills(path) } else { Vec::new() };
        let agents = if agents_dir.exists() { scan_agents(path) } else { Vec::new() };

        if skills.is_empty() && agents.is_empty() {
            continue;
        }

        total_skills += skills.len();
        total_agents += agents.len();

        let project_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        plugins.push(plugin::PluginInfo {
            key: project_path.clone(),
            name: project_name,
            version: String::new(),
            scope: "project".into(),
            marketplace: None,
            install_path: project_path.clone(),
            enabled: true,
            health: "ok".into(),
            health_issues: Vec::new(),
            skills,
            agents,
            installed_at: None,
            last_updated: None,
        });
    }

    Ok(plugin::SkillsOverview {
        total_plugins: plugins.len(),
        total_skills,
        total_agents,
        health_summary: plugin::HealthSummary {
            ok: plugins.len(),
            partial: 0,
            hook: 0,
            broken: 0,
        },
        plugins,
    })
}
```

- [ ] **Step 2: Add helper functions for scanning and JSON reading**

Add these private helpers in the same file (before the command functions):

```rust
fn read_json(path: &std::path::Path) -> Result<serde_json::Value, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("Failed to read {}: {}", path.display(), e)))?;
    serde_json::from_str(&content)
        .map_err(|e| AppError::ParseError(format!("Invalid JSON in {}: {}", path.display(), e)))
}

fn read_json_or_default(path: &std::path::Path) -> serde_json::Value {
    read_json(path).unwrap_or(serde_json::Value::Object(Default::default()))
}

fn scan_skills(plugin_path: &std::path::Path) -> Vec<plugin::SkillInfo> {
    let skills_dir = plugin_path.join("skills");
    if !skills_dir.exists() {
        return Vec::new();
    }
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let skill_md = entry.path().join("SKILL.md");
            if skill_md.exists() {
                if let Some(info) = parse_frontmatter(&skill_md, "skill") {
                    skills.push(info);
                }
            }
        }
    }
    skills
}

fn scan_agents(plugin_path: &std::path::Path) -> Vec<plugin::SkillInfo> {
    let agents_dir = plugin_path.join("agents");
    if !agents_dir.exists() {
        return Vec::new();
    }
    let mut agents = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(info) = parse_frontmatter(&path, "agent") {
                    agents.push(info);
                }
            }
        }
    }
    agents
}

fn has_hooks(plugin_path: &std::path::Path) -> bool {
    let hooks_file = plugin_path.join("hooks/hooks.json");
    hooks_file.exists() || plugin_path.join("hooks").join("session-start").exists()
}

fn parse_frontmatter(path: &std::path::Path, skill_type: &str) -> Option<plugin::SkillInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim_start();

    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let yaml_str = &rest[..end];

    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_str).ok()?;
    let name = yaml["name"].as_str().unwrap_or("").to_string();
    let description = yaml["description"].as_str().unwrap_or("").to_string();
    let tools = yaml["tools"].as_str().map(String::from);

    Some(plugin::SkillInfo {
        name,
        description,
        skill_type: skill_type.into(),
        tools,
        file_path: path.to_string_lossy().into_owned(),
        health: "ok".into(),
        health_detail: None,
    })
}
```

- [ ] **Step 3: Add import for domain::plugin at top of commands.rs**

At the top of `src-tauri/src/app/commands.rs`, add to the existing use block:

```rust
use crate::domain::plugin;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app/commands.rs
git commit -m "feat(skills): implement list_plugins command"
```

---

### Task 4: Implement backend — toggle_plugin and uninstall_plugin commands

**Files:**
- Modify: `src-tauri/src/app/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add toggle_plugin and uninstall_plugin to commands.rs**

Add at the end of `src-tauri/src/app/commands.rs`:

```rust
#[tauri::command]
pub fn toggle_plugin(key: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let settings_path = home.join(".claude/settings.json");

    let mut settings: serde_json::Value = read_json(&settings_path)?;

    let enabled = settings
        .get_mut("enabledPlugins")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| AppError::Internal("No enabledPlugins in settings.json".into()))?;

    let current = enabled.get(&key).and_then(|v| v.as_bool()).unwrap_or(true);
    enabled.insert(key, serde_json::Value::Bool(!current));

    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::Internal(format!("Failed to serialize settings: {}", e)))?;
    std::fs::write(&settings_path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write settings: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(key: String) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Internal("No home directory".into()))?;
    let claude_dir = home.join(".claude");

    // 1. Read registry, find install path, remove directory
    let registry_path = claude_dir.join("plugins/installed_plugins.json");
    let mut registry: serde_json::Value = read_json(&registry_path)?;

    let install_path = registry
        .get("plugins")
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e["installPath"].as_str())
        .ok_or_else(|| AppError::NotFound(format!("Plugin {} not found in registry", key)))?
        .to_string();

    let path = std::path::Path::new(&install_path);
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| AppError::DeleteFailed(format!("Failed to remove {}: {}", install_path, e)))?;
    }

    // 2. Remove from registry
    if let Some(plugins) = registry.get_mut("plugins").and_then(|v| v.as_object_mut()) {
        plugins.remove(&key);
    }
    let output = serde_json::to_string_pretty(&registry)
        .map_err(|e| AppError::Internal(format!("Failed to serialize registry: {}", e)))?;
    std::fs::write(&registry_path, output)
        .map_err(|e| AppError::Internal(format!("Failed to write registry: {}", e)))?;

    // 3. Remove from enabledPlugins in settings.json
    let settings_path = claude_dir.join("settings.json");
    if let Ok(mut settings) = read_json(&settings_path) {
        if let Some(enabled) = settings.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
            enabled.remove(&key);
        }
        if let Ok(output) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&settings_path, output);
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Register all 3 new commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the import block (around line 10-13):

```rust
use app::commands::{list_plugins, toggle_plugin, uninstall_plugin};
```

Wait — the existing import is a single `use app::commands::{ ... }` block. Add the three new names to that existing block.

Then add to the `generate_handler![]` list (around line 79-95):

```rust
list_plugins,
toggle_plugin,
uninstall_plugin,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/app/commands.rs src-tauri/src/lib.rs
git commit -m "feat(skills): implement toggle_plugin and uninstall_plugin commands"
```

---

### Task 5: Add frontend API wrappers

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add types and API functions**

At the bottom of `src/lib/api.ts`, add:

```typescript
// ── Skills / Plugins ──────────────────────────────────────────

export interface SkillInfo {
  name: string;
  description: string;
  skill_type: string;
  tools?: string;
  file_path: string;
  health: string;
  health_detail?: string;
}

export interface MarketplaceInfo {
  name: string;
  repo: string;
  last_updated?: string;
}

export interface PluginInfo {
  key: string;
  name: string;
  version: string;
  scope: string;
  marketplace?: MarketplaceInfo;
  install_path: string;
  enabled: boolean;
  health: string;
  health_issues: string[];
  skills: SkillInfo[];
  agents: SkillInfo[];
  installed_at?: string;
  last_updated?: string;
}

export interface HealthSummary {
  ok: number;
  partial: number;
  hook: number;
  broken: number;
}

export interface SkillsOverview {
  plugins: PluginInfo[];
  total_plugins: number;
  total_skills: number;
  total_agents: number;
  health_summary: HealthSummary;
}

export async function listPlugins(scope: string): Promise<SkillsOverview> {
  return invoke("list_plugins", { scope });
}

export async function togglePlugin(key: string): Promise<void> {
  return invoke("toggle_plugin", { key });
}

export async function uninstallPlugin(key: string): Promise<void> {
  return invoke("uninstall_plugin", { key });
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npm run build`
Expected: builds successfully

- [ ] **Step 3: Commit**

```bash
git add src/lib/api.ts
git commit -m "feat(skills): add frontend API wrappers for plugin commands"
```

---

### Task 6: Add i18n keys

**Files:**
- Modify: `src/i18n/locales/en.json`
- Modify: `src/i18n/locales/zh-CN.json`

- [ ] **Step 1: Add English keys**

Add to `src/i18n/locales/en.json` (before the closing `}`):

```json
  "nav.skills": "Skills",

  "skills.title": "Skills",
  "skills.countChip": "{{skills}} skills \u00b7 {{agents}} agents",
  "skills.global": "Global",
  "skills.project": "Project",
  "skills.viewPlugins": "Plugins",
  "skills.viewAllSkills": "All Skills",
  "skills.health": "Health",
  "skills.healthOk_one": "{{count}} OK",
  "skills.healthOk_other": "{{count}} OK",
  "skills.healthPartial": "{{count}} Partial",
  "skills.healthHook": "{{count}} Hook",
  "skills.healthBroken": "{{count}} Broken",
  "skills.path": "Path",
  "skills.market": "Market",
  "skills.marketUpdated": "Updated {{date}}",
  "skills.uninstall": "Uninstall",
  "skills.uninstallTitle": "Uninstall {{name}}?",
  "skills.uninstallDesc": "This will remove the plugin from disk and delete it from the plugin registry. You can reinstall from the marketplace later.",
  "skills.emptyProject": "No project-level skills found at",
  "skills.emptyStandalone": "No standalone skills found at",
  "skills.moreSkills": "... {{count}} more",
  "skills.disabled": "disabled",
  "skills.scopeGlobal": "Global",
  "skills.scopeProject": "Project",
  "skills.standaloneSection": "Standalone"
```

- [ ] **Step 2: Add Chinese keys**

Add to `src/i18n/locales/zh-CN.json` (before the closing `}`):

```json
  "nav.skills": "技能",

  "skills.title": "技能",
  "skills.countChip": "{{skills}} 个技能 \u00b7 {{agents}} 个代理",
  "skills.global": "全局",
  "skills.project": "项目",
  "skills.viewPlugins": "插件",
  "skills.viewAllSkills": "所有技能",
  "skills.health": "健康",
  "skills.healthOk_other": "{{count}} 正常",
  "skills.healthPartial": "{{count}} 部分",
  "skills.healthHook": "{{count}} 钩子",
  "skills.healthBroken": "{{count}} 损坏",
  "skills.path": "路径",
  "skills.market": "来源",
  "skills.marketUpdated": "更新于 {{date}}",
  "skills.uninstall": "卸载",
  "skills.uninstallTitle": "卸载 {{name}}？",
  "skills.uninstallDesc": "此操作将从磁盘删除插件并从注册表中移除。之后可以从市场重新安装。",
  "skills.emptyProject": "未找到项目级技能",
  "skills.emptyStandalone": "未找到独立技能",
  "skills.moreSkills": "... 还有 {{count}} 个",
  "skills.disabled": "已禁用",
  "skills.scopeGlobal": "全局",
  "skills.scopeProject": "项目",
  "skills.standaloneSection": "独立"
```

- [ ] **Step 3: Verify build**

Run: `npm run build`
Expected: builds successfully

- [ ] **Step 4: Commit**

```bash
git add src/i18n/locales/en.json src/i18n/locales/zh-CN.json
git commit -m "feat(skills): add i18n keys for skills management"
```

---

### Task 7: Add Skills tab to header and routing

**Files:**
- Modify: `src/app/shell/index.tsx`

- [ ] **Step 1: Extend section union type**

In `src/app/shell/index.tsx`, change the section state type at line 25:

```typescript
const [section, setSection] = useState<"sessions" | "skills" | "system">("sessions");
```

- [ ] **Step 2: Update header nav loop**

Change the header nav button loop (around line 64) from:

```tsx
{(["sessions", "system"] as const).map((s) => (
```

to:

```tsx
{(["sessions", "skills", "system"] as const).map((s) => (
```

- [ ] **Step 3: Add conditional render for Skills**

In the `<main>` section (around line 90), add between sessions and system:

```tsx
{section === "skills" && <SkillsPage />}
```

- [ ] **Step 4: Add import at top**

Add to imports at top of file:

```typescript
import SkillsPage from "@/pages/skills/skills-page";
```

- [ ] **Step 5: Verify build**

Run: `npm run build`
Expected: fails because `skills-page.tsx` doesn't exist yet — that's OK, just confirm the import error is the only issue

- [ ] **Step 6: Commit** (will commit together with Task 8)

---

### Task 8: Implement SkillsPage component

**Files:**
- Create: `src/pages/skills/skills-page.tsx`

- [ ] **Step 1: Create the SkillsPage with plugin view, flat view, and uninstall dialog**

Create `src/pages/skills/skills-page.tsx`:

```tsx
import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  listPlugins,
  togglePlugin,
  uninstallPlugin,
  type PluginInfo,
  type SkillInfo,
} from "@/lib/api";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

export default function SkillsPage() {
  const [scope, setScope] = useState<"global" | "project">("global");
  const [view, setView] = useState<"plugin" | "flat">("plugin");
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<PluginInfo | null>(null);
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ["plugins", scope],
    queryFn: () => listPlugins(scope),
  });

  const toggleMut = useMutation({
    mutationFn: (key: string) => togglePlugin(key),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["plugins"] }),
  });

  const uninstallMut = useMutation({
    mutationFn: (key: string) => uninstallPlugin(key),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["plugins"] });
      setUninstallTarget(null);
    },
  });

  const plugins = data?.plugins ?? [];
  const hs = data?.health_summary;
  const flatSkills = plugins.flatMap((p) => [
    ...p.skills.map((s) => ({ ...s, plugin: p })),
    ...p.agents.map((a) => ({ ...a, plugin: p })),
  ]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-border bg-surface px-3 py-2">
        <div className="flex items-center gap-3">
          <h2 className="text-[14px] font-medium text-foreground">{t("skills.title")}</h2>
          <div className="view-toggle flex overflow-hidden rounded-md border border-border">
            <button
              type="button"
              className={`px-2.5 py-1 text-[12px] font-medium transition ${scope === "global" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setScope("global")}
            >
              {t("skills.global")} <span className="text-[10px] opacity-60">{data?.total_plugins ?? "..."}</span>
            </button>
            <button
              type="button"
              className={`border-l border-border px-2.5 py-1 text-[12px] font-medium transition ${scope === "project" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setScope("project")}
            >
              {t("skills.project")} <span className="text-[10px] opacity-60">0</span>
            </button>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="zed-chip px-2 py-1 font-mono text-[12px]">
            {data ? t("skills.countChip", { skills: data.total_skills, agents: data.total_agents }) : "..."}
          </div>
          <div className="view-toggle flex overflow-hidden rounded-md border border-border">
            <button
              type="button"
              className={`px-2.5 py-1 text-[12px] font-medium transition ${view === "plugin" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setView("plugin")}
            >
              {t("skills.viewPlugins")}
            </button>
            <button
              type="button"
              className={`border-l border-border px-2.5 py-1 text-[12px] font-medium transition ${view === "flat" ? "bg-[var(--element-active)] text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              onClick={() => setView("flat")}
            >
              {t("skills.viewAllSkills")}
            </button>
          </div>
        </div>
      </div>

      {/* Health bar */}
      {hs && (
        <div className="flex items-center gap-3 border-b border-[var(--border-variant)] bg-[var(--element)] px-3 py-1.5 text-[12px] text-muted-foreground">
          <span className="text-[10px] uppercase tracking-[0.06em] text-placeholder">{t("skills.health")}</span>
          <HealthDot color="ok" count={hs.ok} />
          <HealthDot color="partial" count={hs.partial} />
          <HealthDot color="hook" count={hs.hook} />
          <HealthDot color="broken" count={hs.broken} />
        </div>
      )}

      {/* Content */}
      <ScrollArea className="min-h-0 flex-1">
        {isLoading ? (
          <div className="space-y-1 p-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-18 w-full rounded-md" />
            ))}
          </div>
        ) : view === "plugin" ? (
          <div className="space-y-1 p-2">
            {plugins.map((p) => (
              <PluginCard
                key={p.key}
                plugin={p}
                expanded={expandedKey === p.key}
                onToggleExpand={() => setExpandedKey(expandedKey === p.key ? null : p.key)}
                onToggle={() => toggleMut.mutate(p.key)}
                onUninstall={() => setUninstallTarget(p)}
              />
            ))}
          </div>
        ) : (
          <div className="p-2">
            {flatSkills.map((s, i) => (
              <div key={i} className="flex items-center gap-2 rounded-md px-3 py-1.5 hover:bg-[var(--element-hover)]">
                <span className={`text-[10px] font-medium uppercase ${s.skill_type === "agent" ? "text-[var(--warning)]" : "text-[var(--accent)]"}`}>
                  {s.skill_type === "agent" ? "A" : "S"}
                </span>
                <span className="w-[160px] shrink-0 truncate text-[13px] font-medium text-foreground">{s.name}</span>
                <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">{s.description}</span>
                <span className="shrink-0 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] px-1.5 py-0.5 text-[11px] text-muted-foreground">
                  {s.plugin.name} ← {s.plugin.marketplace?.name ?? ""}
                </span>
                <span className={`size-1.5 shrink-0 rounded-full ${s.health === "ok" ? "bg-[var(--success)]" : "bg-[var(--warning)]"}`} />
              </div>
            ))}
          </div>
        )}
      </ScrollArea>

      {/* Uninstall dialog */}
      <AlertDialog open={!!uninstallTarget} onOpenChange={(open) => !open && setUninstallTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("skills.uninstallTitle", { name: uninstallTarget?.name })}</AlertDialogTitle>
            <AlertDialogDescription>
              <span className="font-mono text-[11px] text-muted-foreground block mb-2 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] p-2 break-all">
                {uninstallTarget?.install_path}
              </span>
              {t("skills.uninstallDesc")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("detail.deleteCancel")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={uninstallMut.isPending}
              onClick={() => uninstallTarget && uninstallMut.mutate(uninstallTarget.key)}
              className="border-[#4c2b2c] bg-destructive/10 text-destructive hover:bg-destructive/20"
            >
              {uninstallMut.isPending ? t("detail.deleting") : t("skills.uninstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function HealthDot({ color, count }: { color: string; count: number }) {
  const colors: Record<string, string> = {
    ok: "bg-[var(--success)]",
    partial: "bg-[var(--warning)]",
    hook: "bg-[var(--text-placeholder)]",
    broken: "bg-[var(--error)]",
  };
  return (
    <span className="flex items-center gap-1">
      <span className={`size-1.5 rounded-full ${colors[color] ?? ""}`} />
      <span className="font-mono text-[11px]">{count}</span>
    </span>
  );
}

function PluginCard({
  plugin,
  expanded,
  onToggleExpand,
  onToggle,
  onUninstall,
}: {
  plugin: PluginInfo;
  expanded: boolean;
  onToggleExpand: () => void;
  onToggle: () => void;
  onUninstall: () => void;
}) {
  const { t } = useTranslation();
  const borderColors: Record<string, string> = {
    ok: "border-l-[3px] border-l-[var(--success)]",
    partial: "border-l-[3px] border-l-[var(--warning)]",
    hook: "border-l-[3px] border-l-[var(--text-placeholder)]",
    broken: "border-l-[3px] border-l-[var(--error)]",
  };

  return (
    <div className={`overflow-hidden rounded-md border border-border bg-[var(--surface)] transition ${borderColors[plugin.health] ?? ""}`}>
      <div
        className="flex cursor-pointer items-center gap-2 px-3 py-2 hover:bg-[var(--element-hover)]"
        onClick={onToggleExpand}
      >
        <span className={`grid size-4 shrink-0 place-items-center rounded-sm bg-[var(--element)] text-[10px] text-[var(--accent)] transition ${expanded ? "rotate-90" : ""}`}>{"\u25B6"}</span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-foreground">{plugin.name}</p>
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="font-mono">v{plugin.version}</span>
            {(plugin.skills.length + plugin.agents.length) > 0 && (
              <span>{plugin.skills.length} skills, {plugin.agents.length} agents</span>
            )}
            {plugin.health === "hook" && <span>hook-only</span>}
            {plugin.marketplace && (
              <span className="text-[var(--accent)] opacity-70">
                ← {plugin.marketplace.name}
                <span className="font-mono text-[10px] ml-1">{plugin.marketplace.repo}</span>
              </span>
            )}
          </div>
        </div>
        <HealthBadge health={plugin.health} />
        <label className="relative inline-flex shrink-0 cursor-pointer" onClick={(e) => e.stopPropagation()}>
          <input type="checkbox" className="sr-only" checked={plugin.enabled} onChange={onToggle} />
          <span className={`block h-[18px] w-[32px] rounded-full border transition ${plugin.enabled ? "bg-[var(--accent)] border-[var(--accent)]" : "bg-[var(--element-active)] border-[var(--border)]"}`}>
            <span className={`block size-3 rounded-full bg-foreground transition ${plugin.enabled ? "translate-x-[14px]" : ""} mt-[2px] ml-[2px]`} />
          </span>
        </label>
        <button
          type="button"
          className="shrink-0 rounded-md border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground transition hover:border-[var(--error)] hover:text-[var(--error)] hover:bg-[rgba(208,114,119,0.1)]"
          onClick={(e) => { e.stopPropagation(); onUninstall(); }}
        >
          {t("skills.uninstall")}
        </button>
      </div>

      {expanded && (
        <div className="border-t border-border bg-[var(--editor)]">
          <DetailRow label={t("skills.path")} value={plugin.install_path} mono />
          {plugin.marketplace && (
            <DetailRow
              label={t("skills.market")}
              value={`${plugin.marketplace.name} · ${plugin.marketplace.repo}${plugin.marketplace.last_updated ? ` · ${plugin.marketplace.last_updated.split("T")[0]}` : ""}`}
            />
          )}
          {plugin.health_issues.length > 0 && (
            <div className="px-3 py-2 border-b border-[var(--border-variant)]">
              {plugin.health_issues.map((issue, i) => (
                <div key={i} className={`text-[11px] flex items-center gap-1 ${plugin.health === "broken" ? "text-[var(--error)]" : "text-[var(--warning)]"}`}>
                  <span className={`size-1 rounded-full ${plugin.health === "broken" ? "bg-[var(--error)]" : "bg-[var(--warning)]"}`} />
                  {issue}
                </div>
              ))}
            </div>
          )}
          {plugin.skills.map((s) => (
            <SkillRow key={s.name} skill={s} />
          ))}
          {plugin.agents.map((a) => (
            <SkillRow key={a.name} skill={a} />
          ))}
        </div>
      )}
    </div>
  );
}

function DetailRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center gap-2 border-b border-[var(--border-variant)] px-3 py-1">
      <span className="text-[10px] uppercase tracking-[0.06em] text-placeholder opacity-70 shrink-0 w-12">{label}</span>
      <span className={`min-w-0 flex-1 truncate text-[11px] text-placeholder ${mono ? "font-mono direction-rtl text-left" : ""}`}>{value}</span>
    </div>
  );
}

function SkillRow({ skill }: { skill: SkillInfo }) {
  return (
    <div className="flex items-center gap-2 border-b border-[var(--border-variant)] px-3 py-1 hover:bg-[var(--element-hover)]">
      <span className={`grid size-4 shrink-0 place-items-center rounded-sm bg-[var(--element)] text-[9px] ${skill.skill_type === "agent" ? "text-[var(--warning)]" : "text-[var(--accent)]"}`}>
        {skill.skill_type === "agent" ? "A" : "S"}
      </span>
      <span className="text-[13px] text-foreground truncate">{skill.name}</span>
      <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">{skill.description}</span>
      {skill.tools && (
        <span className="shrink-0 rounded-sm border border-[var(--border-variant)] bg-[var(--element)] px-1 py-0.5 font-mono text-[10px] text-muted-foreground">{skill.tools}</span>
      )}
      <span className={`size-1.5 shrink-0 rounded-full ${skill.health === "ok" ? "bg-[var(--success)]" : "bg-[var(--warning)]"}`} />
    </div>
  );
}

function HealthBadge({ health }: { health: string }) {
  const styles: Record<string, string> = {
    ok: "text-[var(--success)] bg-[rgba(161,193,129,0.15)] border-[rgba(161,193,129,0.3)]",
    partial: "text-[var(--warning)] bg-[rgba(222,193,132,0.15)] border-[rgba(222,193,132,0.3)]",
    hook: "text-[var(--text-placeholder)] bg-[rgba(135,138,152,0.15)] border-[rgba(135,138,152,0.3)]",
    broken: "text-[var(--error)] bg-[rgba(208,114,119,0.15)] border-[rgba(208,114,119,0.3)]",
  };
  const dotColors: Record<string, string> = {
    ok: "bg-[var(--success)]",
    partial: "bg-[var(--warning)]",
    hook: "bg-[var(--text-placeholder)]",
    broken: "bg-[var(--error)]",
  };
  const labels: Record<string, string> = {
    ok: "OK",
    partial: "PARTIAL",
    hook: "HOOK",
    broken: "BROKEN",
  };
  return (
    <span className={`flex shrink-0 items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.04em] ${styles[health] ?? ""}`}>
      <span className={`size-1 rounded-full ${dotColors[health] ?? ""}`} />
      {labels[health] ?? health.toUpperCase()}
    </span>
  );
}
```

- [ ] **Step 2: Commit Tasks 7 and 8 together**

```bash
git add src/app/shell/index.tsx src/pages/skills/skills-page.tsx
git commit -m "feat(skills): add Skills tab and SkillsPage component"
```

---

### Task 9: Verify full build and manual test

**Files:** None — verification only

- [ ] **Step 1: Run full Rust check**

Run: `cargo check`
Expected: no errors

- [ ] **Step 2: Run frontend build**

Run: `npm run build`
Expected: builds successfully

- [ ] **Step 3: Start dev server and manually test**

Run: `cargo tauri dev`

Verify:
1. "Skills" tab appears in header between Sessions and System
2. Clicking "Skills" shows the plugin list with health badges
3. Global/Project toggle works
4. Plugin cards expand to show skills, agents, install path, marketplace
5. Toggle switch triggers toggle_plugin command
6. Uninstall button shows AlertDialog, confirms and removes plugin
7. Flat view shows all skills/agents in a flat list
8. Health bar shows correct counts
9. Language toggle shows Chinese translations

- [ ] **Step 4: Commit any fixes if needed**

---

## Self-Review

**Spec coverage:**
- Domain types → Task 1 ✓
- list_plugins (global + project) → Tasks 3 ✓
- toggle_plugin → Task 4 ✓
- uninstall_plugin → Task 4 ✓
- YAML frontmatter parsing → Task 3 ✓
- Health validation (OK/PARTIAL/HOOK/BROKEN) → Task 3 ✓
- Marketplace info inline → Tasks 3, 8 ✓
- Frontend API wrappers → Task 5 ✓
- i18n keys → Task 6 ✓
- Skills tab in header → Task 7 ✓
- SkillsPage with plugin/flat views → Task 8 ✓
- AlertDialog for uninstall → Task 8 ✓
- Global/Project toggle → Task 8 ✓

**Placeholder scan:** No TBD, TODO, or "implement later" found. All code blocks contain actual implementation.

**Type consistency:** `PluginInfo`, `SkillInfo`, `MarketplaceInfo`, `HealthSummary`, `SkillsOverview` — same names and fields across Rust domain types, Rust command code, and TypeScript API types.
