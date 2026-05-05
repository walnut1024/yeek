# Proxy Error Drill-Down Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Click the Dashboard Error metric card to open a Sheet showing recent proxy error events in a feed/timeline.

**Architecture:** vendor_proxy records each error as an `ErrorEvent` in an in-memory ring buffer (cap 100). A new `/admin/errors` endpoint exposes them. yeek main process relays via a new command. Frontend opens a shadcn Sheet to display the feed.

**Tech Stack:** Rust (vendor_proxy: axum, VecDeque), Rust (yeek: ureq), TypeScript/React (Sheet, useQuery)

---

### Task 1: Add ErrorEvent struct and ring buffer to vendor_proxy

**Files:**
- Modify: `vendor_proxy/src/server.rs:27-37` (AppState)
- Modify: `vendor_proxy/src/server.rs:362-390` (Err branch)
- Modify: `vendor_proxy/src/main.rs:27-37` (AppState construction)

- [ ] **Step 1: Add ErrorEvent struct after ProviderStats (server.rs:44)**

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorEvent {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub message: String,
}
```

- [ ] **Step 2: Add error_events field to AppState (server.rs:36, after provider_stats)**

```rust
pub error_events: Mutex<VecDeque<ErrorEvent>>,
```

Also add `use std::collections::VecDeque;` to imports if not already present (it is — used for `request_times`).

- [ ] **Step 3: Add error event recording in the Err branch (server.rs:363, after error_count.fetch_add)**

Insert after `state.error_count.fetch_add(1, Ordering::Relaxed);`:

```rust
{
    let mut events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
    events.push_front(ErrorEvent {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        provider: provider_name.clone(),
        model: responses_req.model.clone(),
        status: match &e {
            crate::client::ProxyError::ProviderError { status, .. } => *status,
            _ => 500,
        },
        message: e.to_string(),
    });
    while events.len() > 100 {
        events.pop_back();
    }
}
```

- [ ] **Step 4: Add error_events field to AppState construction (main.rs:36, after provider_stats)**

```rust
error_events: Mutex::new(VecDeque::new()),
```

- [ ] **Step 5: Add GET /admin/errors handler (server.rs, after admin_status function)**

```rust
pub async fn admin_errors(State(state): State<Arc<AppState>>) -> Json<Vec<ErrorEvent>> {
    let events = state.error_events.lock().unwrap_or_else(|e| e.into_inner());
    Json(events.clone().into_iter().collect())
}
```

- [ ] **Step 6: Register the route (main.rs:41, after /admin/status route)**

```rust
.route("/admin/errors", axum::routing::get(server::admin_errors))
```

- [ ] **Step 7: Build and verify**

Run: `cargo build -p vendor_proxy`
Expected: compiles without errors

- [ ] **Step 8: Commit**

```bash
git add vendor_proxy/src/server.rs vendor_proxy/src/main.rs
git commit -m "feat(proxy): record error events in ring buffer, add /admin/errors endpoint"
```

---

### Task 2: Add get_error_events to yeek ProxyManager

**Files:**
- Modify: `src-tauri/src/app/proxy/mod.rs` (add method)
- Modify: `src-tauri/src/app/commands.rs` (add command)
- Modify: `src-tauri/src/http/routes.rs:60-61` (add route)
- Modify: `src/lib/transport.ts:54-55` (add route mapping)

- [ ] **Step 1: Add ProxyErrorEvent struct and method to ProxyManager (proxy/mod.rs, after ProxyMetrics struct ~line 80)**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyErrorEvent {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub message: String,
}
```

- [ ] **Step 2: Add get_error_events method to ProxyManager impl (proxy/mod.rs, after get_metrics ~line 242)**

```rust
pub fn get_error_events(&self) -> Result<Vec<ProxyErrorEvent>, AppError> {
    let config = self.read_config()?;
    let url = format!("http://{}/admin/errors", config.server.listen_addr);
    let body = ureq::get(&url).call()
        .map_err(|e| AppError::Internal(format!("error events: {}", e)))?
        .into_body().read_to_string()
        .map_err(|e| AppError::Internal(format!("error events read: {}", e)))?;
    serde_json::from_str(&body)
        .map_err(|e| AppError::ParseError(format!("error events json: {}", e)))
}
```

Add `use std::io::Read;` to the file imports if not already present (check line 10 — it's already there).

- [ ] **Step 3: Add command in commands.rs (after do_get_proxy_metrics ~line 1979)**

```rust
pub(crate) fn do_get_proxy_error_events(
    state: &AppState,
) -> Result<Vec<crate::app::proxy::ProxyErrorEvent>, AppError> {
    state.proxy_manager.get_error_events()
}
```

- [ ] **Step 4: Add HTTP route handler (routes.rs, after proxy_logs function ~line 444)**

```rust
async fn proxy_error_events(
    State(state): State<HttpRuntimeState>,
) -> Result<Json<Vec<crate::app::proxy::ProxyErrorEvent>>, AppError> {
    tokio::task::spawn_blocking(move || do_get_proxy_error_events(&state.app_state).map(Json))
        .await
        .unwrap_or_else(|e| Err(AppError::Internal(e.to_string())))
}
```

- [ ] **Step 5: Register the route (routes.rs:61, after /proxy/logs)**

```rust
.route("/proxy/errors", get(proxy_error_events))
```

- [ ] **Step 6: Add transport mapping (transport.ts:55, after get_proxy_logs)**

```ts
get_proxy_error_events: { method: "GET", path: "/api/proxy/errors" },
```

- [ ] **Step 7: Build and verify**

Run: `cargo check`
Expected: compiles without errors

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/app/proxy/mod.rs src-tauri/src/app/commands.rs src-tauri/src/http/routes.rs src/lib/transport.ts
git commit -m "feat: add proxy error events relay endpoint"
```

---

### Task 3: Add frontend API type and Sheet component

**Files:**
- Modify: `src/lib/api.ts` (add type + function)
- Create: `src/components/ui/sheet.tsx` (shadcn Sheet)

- [ ] **Step 1: Add ProxyErrorEvent type and API function (api.ts, after getProxyMetrics ~line 436)**

```ts
export interface ProxyErrorEvent {
  timestamp: number;
  provider: string;
  model: string;
  status: number;
  message: string;
}

export async function getProxyErrorEvents(): Promise<ProxyErrorEvent[]> {
  return getTransport().command<ProxyErrorEvent[]>("get_proxy_error_events");
}
```

- [ ] **Step 2: Add shadcn Sheet component**

Run: `npx shadcn@latest add sheet`

If the CLI fails, create `src/components/ui/sheet.tsx` manually with the standard shadcn Sheet implementation based on `@radix-ui/react-dialog`. Verify `@radix-ui/react-dialog` is installed:

```bash
npm ls @radix-ui/react-dialog
```

If not installed: `npm install @radix-ui/react-dialog`

The Sheet component should export `Sheet`, `SheetTrigger`, `SheetClose`, `SheetContent`, `SheetHeader`, `SheetFooter`, `SheetTitle`, `SheetDescription` — following the standard shadcn pattern.

- [ ] **Step 3: Verify frontend builds**

Run: `npm run build`
Expected: no type errors

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/components/ui/sheet.tsx package.json package-lock.json
git commit -m "feat: add ProxyErrorEvent API type and Sheet component"
```

---

### Task 4: Wire up Error card click → Sheet with error feed

**Files:**
- Modify: `src/pages/dashboard/dashboard-page.tsx`

- [ ] **Step 1: Add imports and state**

Add to imports at top of file:

```tsx
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { getProxyErrorEvents, type ProxyErrorEvent } from "@/lib/api";
```

Add state inside `DashboardPage` component (after `showAllActions` state ~line 18):

```tsx
const [showErrorSheet, setShowErrorSheet] = useState(false);
```

- [ ] **Step 2: Add error events query (after proxy-metrics query ~line 34)**

```tsx
const { data: errorEvents } = useQuery({
  queryKey: ["proxy-error-events"],
  queryFn: getProxyErrorEvents,
  enabled: showErrorSheet,
  refetchInterval: showErrorSheet ? 5000 : false,
});
```

- [ ] **Step 3: Make MetricCard clickable — add onClick prop**

Change `MetricCard` component signature (line 171):

```tsx
function MetricCard({ label, value, sub, danger, onClick }: { label: string; value: string; sub: string; danger?: boolean; onClick?: () => void }) {
```

Update the outer div (line 173) to support click:

```tsx
<div
  className={`border border-border bg-card px-3 py-3.5 text-center ${onClick ? "cursor-pointer hover:bg-card/80 transition-colors" : ""}`}
  onClick={onClick}
>
```

- [ ] **Step 4: Add onClick to the Error MetricCard (line 76)**

Change:
```tsx
<MetricCard label={t("dashboard.metricErrors")} value={String(metrics.error_count)} sub={`${((metrics.error_count / Math.max(metrics.request_count, 1)) * 100).toFixed(2)}%`} danger={metrics.error_count > 0} />
```
To:
```tsx
<MetricCard label={t("dashboard.metricErrors")} value={String(metrics.error_count)} sub={`${((metrics.error_count / Math.max(metrics.request_count, 1)) * 100).toFixed(2)}%`} danger={metrics.error_count > 0} onClick={() => setShowErrorSheet(true)} />
```

- [ ] **Step 5: Add ErrorSheet component at bottom of file (after HealthCard)**

```tsx
function ErrorSheet({ open, onOpenChange, events }: { open: boolean; onOpenChange: (v: boolean) => void; events?: ProxyErrorEvent[] }) {
  const { t } = useTranslation();
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-[420px] sm:max-w-[420px] flex flex-col">
        <SheetHeader>
          <SheetTitle className="text-[14px] font-medium">Proxy Errors</SheetTitle>
          <SheetDescription className="text-[12px] text-muted-foreground">
            Retains last 100 errors, cleared on proxy restart
          </SheetDescription>
        </SheetHeader>
        <div className="flex-1 overflow-auto px-1 pt-2">
          {!events || events.length === 0 ? (
            <p className="py-8 text-center text-[13px] text-muted-foreground">{t("dashboard.noActions")}</p>
          ) : (
            events.map((e, i) => (
              <div key={i} className="border-b border-border py-2.5">
                <div className="flex items-center gap-2">
                  <span className={`font-mono text-[11px] font-medium px-1.5 py-0.5 rounded ${e.status >= 500 ? "bg-destructive/10 text-destructive" : "bg-amber-400/10 text-amber-500"}`}>
                    {e.status}
                  </span>
                  <span className="font-mono text-[11px] text-foreground/60">{e.provider}</span>
                  <span className="font-mono text-[11px] text-muted-foreground truncate">{e.model}</span>
                  <span className="ml-auto shrink-0 font-mono text-[11px] text-muted-foreground">
                    {new Date(e.timestamp).toLocaleTimeString(getCurrentLocale(), { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false })}
                  </span>
                </div>
                <p className="mt-1 text-[12px] text-foreground/60 leading-[1.4] break-all">{e.message}</p>
              </div>
            ))
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
```

- [ ] **Step 6: Render ErrorSheet in DashboardPage (before closing `</div>` of the page, ~line 155)**

```tsx
<ErrorSheet open={showErrorSheet} onOpenChange={setShowErrorSheet} events={errorEvents} />
```

- [ ] **Step 7: Build and verify**

Run: `npm run build`
Expected: no type errors

- [ ] **Step 8: Visual test**

Run: `cargo tauri dev`
Verify: Error card is clickable, Sheet opens from right, shows error feed or empty state.

- [ ] **Step 9: Commit**

```bash
git add src/pages/dashboard/dashboard-page.tsx
git commit -m "feat: clickable error card opens Sheet with proxy error feed"
```
