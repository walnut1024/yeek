# Proxy Error Drill-Down

Click the Error metric card on the Dashboard to open a Sheet showing recent proxy error events in a timeline/feed format.

## Scope

- **Only proxy errors** (vendor_proxy forwarding failures to LLM providers)
- **In-memory ring buffer** (cap 100), cleared on proxy restart
- No persistence, no historical queries

## Data Source

### ErrorEvent struct (vendor_proxy/src/server.rs)

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorEvent {
    pub timestamp: u64,       // unix millis
    pub provider: String,
    pub model: String,
    pub status: u16,
    pub message: String,
}
```

### Storage

- `AppState.error_events: Mutex<VecDeque<ErrorEvent>>` (cap 100, oldest evicted)
- Populated in `proxy_handler`'s `Err(e)` branch (server.rs:362), alongside existing `error_count.fetch_add(1)`

### New endpoint

`GET /admin/errors` returns `Vec<ErrorEvent>` sorted newest-first.

## Backend: yeek main process

- `ProxyManager::get_error_events()` — HTTP GET to `http://{listen_addr}/admin/errors`, parse JSON
- New Tauri command `get_proxy_error_events` in `commands.rs`, delegates to `ProxyManager`
- Route: `GET /api/proxy/errors` in `routes.rs`

## Frontend

### API layer (src/lib/api.ts)

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

### UI (src/pages/dashboard/dashboard-page.tsx)

- `MetricCard` gains `onClick?: () => void` prop
- Error card passes `onClick={() => setShowErrorSheet(true)}`
- Error card gets `cursor-pointer` and hover state when `error_count > 0`
- Sheet (shadcn Sheet, side="right") contains:
  - Header: "Proxy Errors" with close button
  - Content: scrollable feed of error events, each showing:
    - Relative time + absolute time
    - Provider name
    - Model
    - HTTP status (colored badge: 4xx amber, 5xx red)
    - Error message (monospace, truncated with expand)
  - Empty state when no errors
  - Footer note: "Retains last 100 errors, cleared on proxy restart"

### Data fetching

- Error events fetched when Sheet opens (useQuery with `enabled: showSheet`)
- Refetch every 5s while Sheet is open (aligns with existing proxy metrics polling)

## Files Changed

| File | Change |
|------|--------|
| `vendor_proxy/src/server.rs` | Add `ErrorEvent` struct, `error_events` field, push on error, `GET /admin/errors` handler |
| `src-tauri/src/app/proxy/mod.rs` | Add `get_error_events()` method |
| `src-tauri/src/app/commands.rs` | Add `get_proxy_error_events` command |
| `src-tauri/src/http/routes.rs` | Add `GET /api/proxy/errors` route |
| `src/lib/api.ts` | Add `ProxyErrorEvent` type and `getProxyErrorEvents()` |
| `src/pages/dashboard/dashboard-page.tsx` | Clickable error card, Sheet with error feed |
| `src/components/ui/sheet.tsx` | Add shadcn Sheet component (if not exists) |
