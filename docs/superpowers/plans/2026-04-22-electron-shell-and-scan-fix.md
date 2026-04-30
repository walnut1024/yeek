# Electron Shell + Scan Corruption Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a minimal Electron shell alongside the existing Tauri app and diagnose/fix the 322 scan errors in the JSONL indexer.

**Architecture:** Electron main process spawns the existing `yeek-server` HTTP binary, loads the shared frontend via custom `yeek://` protocol (prod) or Vite dev server (dev). Scan diagnostics added as a `--diagnose-scan` CLI flag that runs a separate diagnostic pass classifying errors by pipeline stage.

**Tech Stack:** Electron 35, electron-builder, TypeScript, Rust (existing yeek-server binary), axum CORS.

**Spec:** `docs/superpowers/specs/2026-04-22-electron-shell-and-scan-fix-design.md`

---

## File Structure

### Created

| File | Responsibility |
|---|---|
| `electron-app/package.json` | Electron dependencies and scripts |
| `electron-app/tsconfig.json` | TypeScript config for Electron main process |
| `electron-app/src/main.ts` | Electron main process: server lifecycle, window, custom protocol |
| `electron-app/src/preload.ts` | Minimal preload script |
| `electron-app/electron-builder.yml` | Packaging config for macOS |
| `src-tauri/src/adapter/claudecode/diagnostic.rs` | Scan diagnostic types and entry point |

### Modified

| File | Change |
|---|---|
| `package.json` | Add `electron:dev`, `electron:compile`, `electron:build` scripts |
| `src-tauri/src/http/routes.rs` | Add `yeek://localhost` to CORS origins |
| `src-tauri/src/adapter/claudecode/mod.rs` | Register `diagnostic` module |
| `src-tauri/src/bin/server.rs` | Add `--diagnose-scan` CLI flag |

---

## Task 1: Baseline Verification

**Files:** None (verification only)

- [ ] **Step 1: Verify root Vite dev server works**

Run: `npm run dev`
Expected: Vite starts on `http://localhost:1420` without errors. Ctrl+C to stop.

- [ ] **Step 2: Verify Tauri dev builds**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished dev [unoptimized + debuginfo]` with no errors.

- [ ] **Step 3: Verify yeek-server binary builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server`
Expected: `Finished dev` with no errors. Binary at `src-tauri/target/debug/yeek-server`.

- [ ] **Step 4: Record scan error baseline**

Run: `./src-tauri/target/debug/yeek-server &` then `sleep 15 && curl -s http://127.0.0.1:17321/api/system/status | python3 -m json.tool`, then kill the server.
Expected: Note the error count from the scan log output for later comparison.

---

## Task 2: Electron Project Scaffolding

**Files:**
- Create: `electron-app/package.json`
- Create: `electron-app/tsconfig.json`
- Create: `electron-app/electron-builder.yml`

- [ ] **Step 1: Create electron-app directory**

Run: `mkdir -p electron-app/src`

- [ ] **Step 2: Create `electron-app/package.json`**

```json
{
  "name": "yeek-electron",
  "version": "2.0.0-alpha.1",
  "private": true,
  "main": "dist/main.js",
  "scripts": {
    "build": "tsc -p tsconfig.json"
  },
  "devDependencies": {
    "electron": "^35.0.0",
    "electron-builder": "^26.0.0",
    "@types/node": "^24.12.2"
  }
}
```

- [ ] **Step 3: Install Electron dependencies**

Run: `cd electron-app && npm install && cd ..`

Note: `npm install` inside `electron-app/` downloads `electron` and `electron-builder`. This may take a minute.

- [ ] **Step 4: Create `electron-app/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2023",
    "module": "commonjs",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "declaration": false,
    "sourceMap": true
  },
  "include": ["src/**/*"]
}
```

- [ ] **Step 5: Create `electron-app/electron-builder.yml`**

```yaml
appId: dev.yeek.app.electron
productName: Yeek
directories:
  output: release
files:
  - electron-app/dist/**/*
  - dist/**/*
  - package.json
extraResources:
  - from: src-tauri/target/release/yeek-server
    to: yeek-server
mac:
  category: public.app-category.developer-tools
  target:
    - dmg
    - zip
  executableName: Yeek
```

- [ ] **Step 6: Verify TypeScript compiles (with empty source)**

Create a placeholder:
```bash
echo "console.log('hello');" > electron-app/src/main.ts
cd electron-app && npx tsc && cd ..
```

Expected: `electron-app/dist/main.js` created. Then `rm electron-app/src/main.ts`.

- [ ] **Step 7: Commit**

```bash
git add electron-app/package.json electron-app/tsconfig.json electron-app/electron-builder.yml electron-app/package-lock.json
git commit -m "chore: scaffold electron-app project structure"
```

---

## Task 3: Electron Main Process

**Files:**
- Create: `electron-app/src/main.ts`
- Create: `electron-app/src/preload.ts`

- [ ] **Step 1: Create `electron-app/src/main.ts`**

```typescript
import { app, BrowserWindow, protocol, net } from "electron";
import { ChildProcess, spawn } from "child_process";
import path from "path";
import http from "http";

const SERVER_PORT = 17321;
const VITE_DEV_URL = "http://localhost:1420";
const READINESS_RETRIES = 20;
const READINESS_INTERVAL_MS = 500;

let serverProcess: ChildProcess | null = null;
let mainWindow: BrowserWindow | null = null;

// Register custom protocol before app is ready (required by Electron)
protocol.registerSchemesAsPrivileged([
  {
    scheme: "yeek",
    privileges: {
      secure: true,
      standard: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

function getServerPath(): string {
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "yeek-server");
  }
  return path.resolve(__dirname, "..", "..", "src-tauri", "target", "debug", "yeek-server");
}

function startServer(): ChildProcess {
  const serverPath = getServerPath();
  console.log(`[yeek-electron] Starting server: ${serverPath}`);

  const proc = spawn(serverPath, [], { stdio: ["ignore", "pipe", "pipe"] });

  proc.stdout?.on("data", (data: Buffer) => {
    console.log(`[yeek-server] ${data.toString().trim()}`);
  });
  proc.stderr?.on("data", (data: Buffer) => {
    console.error(`[yeek-server] ${data.toString().trim()}`);
  });
  proc.on("error", (err) => {
    console.error("[yeek-electron] Failed to start yeek-server:", err);
    app.quit();
  });

  return proc;
}

function waitForServer(): Promise<void> {
  return new Promise((resolve, reject) => {
    let attempts = 0;
    const tryConnect = () => {
      if (attempts >= READINESS_RETRIES) {
        reject(new Error(`Server not ready after ${(READINESS_RETRIES * READINESS_INTERVAL_MS) / 1000}s`));
        return;
      }
      attempts++;
      const req = http.get(
        `http://127.0.0.1:${SERVER_PORT}/api/system/status`,
        (res) => {
          if (res.statusCode === 200) {
            res.resume();
            resolve();
          } else {
            setTimeout(tryConnect, READINESS_INTERVAL_MS);
          }
        },
      );
      req.on("error", () => setTimeout(tryConnect, READINESS_INTERVAL_MS));
      req.setTimeout(2000, () => {
        req.destroy();
        setTimeout(tryConnect, READINESS_INTERVAL_MS);
      });
    };
    tryConnect();
  });
}

function registerProtocol() {
  protocol.handle("yeek", (request) => {
    const url = new URL(request.url);
    // url.pathname is "/index.html" or "/assets/main.js", etc.
    const filePath = path.join(__dirname, "..", "dist", url.pathname);
    return net.fetch(`file://${filePath}`);
  });
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    title: "Yeek",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (app.isPackaged) {
    mainWindow.loadURL("yeek://localhost/index.html");
  } else {
    mainWindow.loadURL(VITE_DEV_URL);
  }

  mainWindow.on("closed", () => {
    mainWindow = null;
  });
}

function killServer() {
  if (serverProcess) {
    serverProcess.kill();
    serverProcess = null;
  }
}

app.on("ready", async () => {
  try {
    registerProtocol();
    serverProcess = startServer();
    console.log("[yeek-electron] Waiting for server...");
    await waitForServer();
    console.log("[yeek-electron] Server ready, creating window");
    createWindow();
  } catch (err) {
    console.error("[yeek-electron] Startup failed:", err);
    killServer();
    app.quit();
  }
});

app.on("window-all-closed", () => {
  killServer();
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (mainWindow === null) {
    createWindow();
  }
});

app.on("before-quit", () => {
  killServer();
});
```

- [ ] **Step 2: Create `electron-app/src/preload.ts`**

```typescript
import { contextBridge } from "electron";

contextBridge.exposeInMainWorld("electronAPI", {
  isElectron: true,
});
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd electron-app && npx tsc && cd ..`

Expected: No errors. `electron-app/dist/main.js` and `electron-app/dist/preload.js` created.

- [ ] **Step 4: Commit**

```bash
git add electron-app/src/main.ts electron-app/src/preload.ts electron-app/dist/
git commit -m "feat(electron): add main process with server lifecycle and custom protocol"
```

---

## Task 4: Root Scripts and CORS

**Files:**
- Modify: `package.json` (add scripts)
- Modify: `src-tauri/src/http/routes.rs` (CORS origins)

- [ ] **Step 1: Add Electron scripts to root `package.json`**

Add these scripts to the `"scripts"` section in `package.json`:

```json
"electron:compile": "tsc -p electron-app/tsconfig.json",
"electron:dev": "npm run electron:compile && concurrently \"npm run dev\" \"electron electron-app/dist/main.js\"",
"electron:build": "npm run electron:compile && npm run build && cargo build --release --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server && electron-builder --config electron-app/electron-builder.yml"
```

Also add `concurrently` as a devDependency:

```json
"concurrently": "^9.1.2"
```

- [ ] **Step 2: Install concurrently**

Run: `npm install --save-dev concurrently`

- [ ] **Step 3: Add `yeek://localhost` to CORS origins in `src-tauri/src/http/routes.rs`**

In the `cors_layer` function (line 62), change the `origins` vector:

```rust
fn cors_layer() -> CorsLayer {
    let origins = vec![
        "http://localhost:1420".parse().unwrap(),  // Vite dev
        "http://localhost:17321".parse().unwrap(), // self
        "tauri://localhost".parse().unwrap(),       // Tauri
        "yeek://localhost".parse().unwrap(),        // Electron production
    ];
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
}
```

- [ ] **Step 4: Verify Rust compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --features http-server`

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add package.json package-lock.json src-tauri/src/http/routes.rs
git commit -m "feat(electron): add root scripts and CORS origin for yeek:// protocol"
```

---

## Task 5: Electron Dev Verification

**Files:** None (testing only)

- [ ] **Step 1: Build yeek-server binary**

Run: `cargo build --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server`

Expected: Binary at `src-tauri/target/debug/yeek-server`.

- [ ] **Step 2: Make sure no other yeek-server is running**

Run: `lsof -i :17321`

If any process is using port 17321, kill it: `kill <PID>`.

- [ ] **Step 3: Start Electron dev mode**

Run: `npm run electron:dev`

Expected:
1. Vite dev server starts on `http://localhost:1420`
2. Electron `main.ts` spawns `yeek-server`
3. After readiness check, Electron window opens
4. Frontend renders with full UI

- [ ] **Step 4: Verify API connectivity**

In the Electron window:
- Navigate to Sessions page — should load session list via HTTP transport
- Navigate to System page — should show system status
- Check browser DevTools console (Cmd+Option+I) for errors — there should be none

- [ ] **Step 5: Verify SSE events**

In the Electron window:
- Trigger a rescan from System page
- Observe sync progress appearing in the UI
- This confirms SSE transport works in Electron

- [ ] **Step 6: Verify shutdown**

Close the Electron window. Check that `yeek-server` process was killed:

Run: `lsof -i :17321`

Expected: No process on port 17321.

---

## Task 6: Electron Production Packaging

**Files:**
- Modify: `electron-app/electron-builder.yml` (if adjustments needed)

- [ ] **Step 1: Run the production build**

Run: `npm run electron:build`

This command:
1. Compiles `electron-app/src/main.ts` → `electron-app/dist/main.js`
2. Builds frontend → `dist/`
3. Builds `yeek-server` in release mode
4. Packages everything with `electron-builder`

Expected: DMG and ZIP in `release/` directory.

- [ ] **Step 2: Test the packaged app**

Open the DMG or run the app from `release/mac-arm64/` (or `mac/`).

Expected:
1. App launches
2. `yeek-server` starts (check Activity Monitor for the process)
3. Frontend loads from `yeek://localhost/index.html`
4. API requests to `http://127.0.0.1:17321/api/*` succeed
5. SSE events received

- [ ] **Step 3: Verify CORS in production**

In the running packaged app, open DevTools and check the Network tab:
- API calls should return 200, not CORS errors
- SSE connection to `/api/events` should be established

If CORS errors appear, verify that the `yeek-server` binary bundled in the app includes the `yeek://localhost` CORS origin (i.e., was built from the updated `routes.rs`).

- [ ] **Step 4: Verify shutdown in packaged app**

Quit the app normally. Check Activity Monitor for orphaned `yeek-server` processes.

Expected: No orphaned processes.

- [ ] **Step 5: Commit packaging fixes (if any)**

If any adjustments were needed to `electron-builder.yml` or `main.ts`:

```bash
git add electron-app/
git commit -m "fix(electron): adjust production packaging"
```

---

## Task 7: Scan Diagnostic Types

**Files:**
- Create: `src-tauri/src/adapter/claudecode/diagnostic.rs`
- Modify: `src-tauri/src/adapter/claudecode/mod.rs` (add `mod diagnostic`)

- [ ] **Step 1: Create `src-tauri/src/adapter/claudecode/diagnostic.rs`**

```rust
use serde::Serialize;
use std::collections::HashMap;

use crate::app::errors::AppError;
use super::{discover_sources, index_single_source};
use crate::store::schema::configure_connection;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStage {
    Discover,
    FingerprintLoad,
    Parse,
    SessionUpsert,
    MessageUpsert,
    SourceUpssert,
    SourceLink,
    FtsRebuild,
    Commit,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanErrorDetail {
    pub source_path: String,
    pub stage: ScanStage,
    pub error_kind: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ScanDiagnosticResult {
    pub total_discovered: usize,
    pub total_attempted: usize,
    pub total_succeeded: usize,
    pub total_skipped: usize,
    pub total_failed: usize,
    pub failures: Vec<ScanErrorDetail>,
    pub failure_summary: HashMap<String, usize>,
}

/// Classify an AppError into a pipeline stage based on error variant and message.
fn classify_error(e: &AppError) -> (ScanStage, String) {
    match e {
        AppError::ParseError(_) => (ScanStage::Parse, "parse_error".to_string()),
        AppError::DbError(msg) => {
            let stage = if msg.contains("fts5") || msg.contains("messages_fts") {
                ScanStage::FtsRebuild
            } else if msg.contains("sessions") && (msg.contains("INSERT") || msg.contains("UPDATE")) {
                ScanStage::SessionUpsert
            } else if msg.contains("messages") && (msg.contains("INSERT") || msg.contains("UPDATE")) {
                ScanStage::MessageUpsert
            } else if msg.contains("sources") {
                ScanStage::SourceUpssert
            } else if msg.contains("session_sources") {
                ScanStage::SourceLink
            } else {
                ScanStage::Unknown
            };
            (stage, "db_error".to_string())
        }
        AppError::Internal(msg) => {
            if msg.contains("subagent") || msg.contains("Invalid") {
                (ScanStage::Parse, "path_parse_error".to_string())
            } else {
                (ScanStage::Unknown, "internal".to_string())
            }
        }
        _ => (ScanStage::Unknown, "other".to_string()),
    }
}

/// Run a diagnostic scan that collects structured error details.
/// Opens its own connection and does NOT persist changes (rolls back at the end).
pub fn run_diagnostic_scan(db_path: &std::path::Path) -> Result<ScanDiagnosticResult, AppError> {
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| AppError::DbError(e.to_string()))?;
    configure_connection(&conn)?;

    // 1. Discover
    let sources = discover_sources()?;
    let total_discovered = sources.len();

    if total_discovered == 0 {
        return Ok(ScanDiagnosticResult {
            total_discovered: 0,
            total_attempted: 0,
            total_succeeded: 0,
            total_skipped: 0,
            total_failed: 0,
            failures: Vec::new(),
            failure_summary: HashMap::new(),
        });
    }

    // 2. Load fingerprints
    let existing_fingerprints: std::collections::HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT path, fingerprint FROM sources WHERE status = 'active'")
            .map_err(|e| AppError::DbError(e.to_string()))?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| AppError::DbError(e.to_string()))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 3. Run scan in a transaction we will ROLLBACK (don't persist)
    conn.execute_batch("BEGIN").map_err(|e| AppError::DbError(e.to_string()))?;

    let mut succeeded = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<ScanErrorDetail> = Vec::new();

    for (i, source) in sources.iter().enumerate() {
        // Skip unchanged
        if let Some(stored_fp) = existing_fingerprints.get(&source.path) {
            if *stored_fp == source.fingerprint {
                skipped += 1;
                continue;
            }
        }

        let sp = format!("dsp_{}", i);
        conn.execute_batch(&format!("SAVEPOINT {}", sp))
            .map_err(|e| AppError::DbError(e.to_string()))?;

        match index_single_source(&conn, source, &existing_fingerprints) {
            Ok(_) => {
                let _ = conn.execute_batch(&format!("RELEASE {}", sp));
                succeeded += 1;
            }
            Err(e) => {
                let _ = conn.execute_batch(&format!("ROLLBACK TO {}", sp));
                let (stage, error_kind) = classify_error(&e);
                errors.push(ScanErrorDetail {
                    source_path: source.path.to_string_lossy().to_string(),
                    stage,
                    error_kind,
                    message: e.to_string(),
                });
            }
        }
    }

    // Rollback everything — diagnostic scan should not modify DB
    conn.execute_batch("ROLLBACK").map_err(|e| AppError::DbError(e.to_string()))?;

    // 4. Summarize
    let mut summary: HashMap<String, usize> = HashMap::new();
    for err in &errors {
        let key = format!("{:?}:{}", err.stage, err.error_kind);
        *summary.entry(key).or_insert(0) += 1;
    }

    let total_failed = errors.len();
    let total_attempted = succeeded + total_failed;

    Ok(ScanDiagnosticResult {
        total_discovered,
        total_attempted,
        total_succeeded: succeeded,
        total_skipped: skipped,
        total_failed,
        failures: errors,
        failure_summary: summary,
    })
}
```

- [ ] **Step 2: Register the module in `src-tauri/src/adapter/claudecode/mod.rs`**

Add at the top of the file (after the existing `use` statements, around line 11):

```rust
pub mod diagnostic;
```

- [ ] **Step 3: Verify Rust compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --features http-server`

Expected: No errors. (There may be unused import warnings — that's fine.)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/adapter/claudecode/diagnostic.rs src-tauri/src/adapter/claudecode/mod.rs
git commit -m "feat(scan): add diagnostic types and classification for scan errors"
```

---

## Task 8: Diagnostic CLI Entry Point

**Files:**
- Modify: `src-tauri/src/bin/server.rs` (add `--diagnose-scan` flag)

- [ ] **Step 1: Add diagnostic flag to `src-tauri/src/bin/server.rs`**

Replace the entire file with:

```rust
use std::sync::Arc;
use yeek_lib::app::state::AppState;
use yeek_lib::http::{HttpRuntimeState, SseEventEmitter, build_router};
use yeek_lib::store::schema;
use yeek_lib::sync::background::ScanGuard;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Diagnostic mode: run scan diagnostics and exit
    if args.len() > 1 && args[1] == "--diagnose-scan" {
        run_diagnostics(&args);
        return;
    }

    // Normal server mode
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("yeek-server starting...");

    // DB init
    let db_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yeek");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("yeek.db");
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open database");
    schema::init_schema(&conn).expect("failed to initialize schema");

    let sse = Arc::new(SseEventEmitter::new());
    let emitter: Arc<dyn yeek_lib::app::events::EventEmitter> = sse.clone();
    let scan_guard = Arc::new(ScanGuard::new());

    // File watcher
    let claude_projects_dir = dirs::home_dir()
        .expect("Cannot find home directory")
        .join(".claude")
        .join("projects");
    let watcher = yeek_lib::sync::watcher::FileWatcher::start(
        claude_projects_dir, db_path.clone(), emitter.clone(), scan_guard.clone(),
    ).expect("Failed to start file watcher");

    let config_watcher = yeek_lib::sync::watcher::FileWatcher::start_plugin_config_watcher(
        emitter.clone(),
    ).expect("Failed to start plugin config watcher");

    let app_state = Arc::new(
        AppState::new(conn, db_path.clone(), emitter)
            .with_watcher(watcher)
            .with_config_watcher(config_watcher),
    );

    // Startup sync
    yeek_lib::sync::background::spawn_background_scan(
        db_path, app_state.event_emitter.clone(), scan_guard,
    );

    // Router
    let runtime_state = HttpRuntimeState { app_state, sse };
    let app = build_router(runtime_state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 17321));
    log::info!("yeek-server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn run_diagnostics(args: &[String]) {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let db_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yeek");
    let db_path = db_dir.join("yeek.db");

    if !db_path.exists() {
        eprintln!("Database not found at {}. Run yeek-server normally first.", db_path.display());
        std::process::exit(1);
    }

    log::info!("Running diagnostic scan on {}...", db_path.display());

    let use_json = args.iter().any(|a| a == "--json");

    match yeek_lib::adapter::claudecode::diagnostic::run_diagnostic_scan(&db_path) {
        Ok(result) => {
            if use_json {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                println!("=== Scan Diagnostic Report ===");
                println!("Total discovered: {}", result.total_discovered);
                println!("Total attempted:  {}", result.total_attempted);
                println!("Succeeded:        {}", result.total_succeeded);
                println!("Skipped (cached): {}", result.total_skipped);
                println!("Failed:           {}", result.total_failed);
                println!();
                if result.total_failed > 0 {
                    println!("--- Failure Summary ---");
                    for (key, count) in &result.failure_summary {
                        println!("  {}: {}", key, count);
                    }
                    println!();
                    println!("--- Sample Failures (first 10) ---");
                    for err in result.failures.iter().take(10) {
                        println!("  [{:?}] {}", err.stage, err.source_path);
                        println!("    kind={}, msg={}", err.error_kind, err.message);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Diagnostic scan failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 2: Verify Rust compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server`

Expected: No errors.

- [ ] **Step 3: Build the diagnostic binary**

Run: `cargo build --manifest-path src-tauri/Cargo.toml --bin yeek-server --features http-server`

Expected: Binary compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit -m "feat(scan): add --diagnose-scan CLI flag with human and JSON output"
```

---

## Task 9: Run Diagnostics and Analyze

**Files:** None (investigation only)

- [ ] **Step 1: Run diagnostic scan**

Run: `./src-tauri/target/debug/yeek-server --diagnose-scan`

Expected: Structured output with total discovered, attempted, succeeded, failed counts, plus failure summary grouped by stage and error kind.

- [ ] **Step 2: Save JSON output for analysis**

Run: `./src-tauri/target/debug/yeek-server --diagnose-scan --json > /tmp/yeek-diagnostic.json`

- [ ] **Step 3: Analyze failure patterns**

Read the output and answer:
1. How many of the 322 errors are parse failures vs DB write failures vs FTS failures?
2. What is the dominant error kind?
3. Are the errors concentrated in specific source types (main sessions vs subagent transcripts)?

- [ ] **Step 4: Manually inspect 2-3 failing JSONL files**

Pick source paths from the sample failures and inspect:

Run: `head -5 <failing-file-path>` and `tail -5 <failing-file-path>`

Look for:
- Truncated/empty lines at the end
- Non-JSON content
- Unexpected schema shapes
- Very large single-line entries

- [ ] **Step 5: Document findings**

Record the root cause in a comment or note. The findings determine the fix strategy in Task 10.

**Possible outcomes:**

| Finding | Next Step |
|---|---|
| Trailing empty/malformed lines in JSONL | Add trailing-line tolerance to parser |
| Unexpected message schema variants | Add variant handling with fixture tests |
| DB write failure (field too long, encoding) | Truncate/sanitize before write |
| FTS rebuild failure on specific content | Sanitize FTS input or skip problematic entries |
| Path parsing bug for subagent sessions | Fix path extraction logic |

---

## Task 10: Targeted Scan Fix

**Files:** TBD based on Task 9 findings

This task implements the fix identified in Task 9. The specific code depends on the diagnostic findings. The implementer should:

- [ ] **Step 1: Implement the smallest fix addressing the observed root cause**

Based on the most common failure pattern from Task 9:

- If parse failures from malformed trailing lines: add tolerance in `parse_session` (around the JSONL line loop)
- If schema variant failures: add pattern matching for the observed variants
- If DB write failures: add truncation/sanitization before upsert
- If FTS failures: add content sanitization for FTS input

- [ ] **Step 2: Add or update tests for the failing scenario**

Add a `#[test]` in `src-tauri/src/adapter/claudecode/mod.rs` (the existing `tests` module) that reproduces the failure and verifies the fix.

- [ ] **Step 3: Re-run diagnostics to measure improvement**

Run: `./src-tauri/target/debug/yeek-server --diagnose-scan`

Expected: Error count materially reduced from the 322 baseline. Any remaining errors should be classified and understood.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/adapter/claudecode/
git commit -m "fix(scan): <description based on actual fix>"
```

---

## Task 11: Final Regression Validation

**Files:** None (verification only)

- [ ] **Step 1: Verify frontend build**

Run: `npm run build`

Expected: No type errors, Vite build succeeds.

- [ ] **Step 2: Verify Tauri still works**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: No errors.

- [ ] **Step 3: Verify Electron dev mode**

Run: `npm run electron:dev`

Verify: Window opens, sessions load, rescan works, SSE events received.

- [ ] **Step 4: Verify scan error count reduced**

Run: `./src-tauri/target/debug/yeek-server --diagnose-scan`

Verify: Error count is lower than the 322 baseline.

- [ ] **Step 5: Smoke checklist**

All of the following should work in both Tauri and Electron:
- [ ] Sessions list loads
- [ ] Session detail loads
- [ ] Session transcript loads
- [ ] Rescan action works
- [ ] Action log works
- [ ] SSE sync progress appears
- [ ] Marketplace tab loads
