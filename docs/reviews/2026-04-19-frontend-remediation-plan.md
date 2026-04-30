# Frontend Remediation Plan — Yeek

**日期**: 2026-04-19
**来源**: `docs/reviews/2026-04-19-frontend-review-spec.md`
**目标**: 按优先级逐项修复前端问题，每项包含具体文件、修改方案与验证方式

---

## Phase 1: 交互语义修正（P0）

### T1. SessionRow 嵌套 button 拆分

**问题**: 外层 `<button>` 包裹内层勾选 `<button>`，HTML 语义无效
**文件**: `src/pages/sessions/session-row.tsx:25-87`

**方案**: 外层 `<button>` 改为 `<div role="button" tabIndex={0} onClick={onSelect} onKeyDown={...}>`，内层勾选保持 `<button>`。添加 `onKeyDown` 处理 Enter/Space 触发选择。

**验证**:
- `tsc -b --noEmit` 通过
- 浏览器中 SessionRow 点击选择正常
- Manage mode 下勾选与行选择互不干扰
- 键盘 Tab 可聚焦行，Enter 可选择

---

### T2. 收敛原生 `<button>` 到 Button 组件

**问题**: 16 处原生 `<button>` 违反 CLAUDE.md "Never use raw `<button>`" 规则
**涉及文件与行号**:

| 文件 | 行号 | 用途 | 改法 |
|------|------|------|------|
| `shell/index.tsx` | 62, 81 | 导航切换、语言切换 | `Button variant="ghost" size="sm"` |
| `shell/index.tsx` | 325, 336 | 排序切换、管理模式 | `Button variant="ghost" size="sm"` |
| `shell/index.tsx` | 402 | 全选 checkbox | 保留 `<button>`（复选框语义不适合 Button） |
| `shell/index.tsx` | 441, 465 | 项目折叠、项目 checkbox | 441→保留（折叠按钮），465→保留（checkbox） |
| `session-detail-pane.tsx` | 161, 167 | Feed/Graph toggle | `Button variant="ghost" size="sm"` + pill-tab class |
| `session-graph.tsx` | 85 | Fit View | `Button variant="outline" size="sm"` |
| `graph/node-detail-panel.tsx` | 61 | 关闭面板 | `Button variant="ghost" size="icon-sm"` |
| `tool-accordion.tsx` | 75, 158 | 工具折叠展开 | 保留（可折叠 header 按钮有特殊样式需求） |
| `error-boundary.tsx` | 40 | 重试按钮 | `Button variant="outline" size="sm"`（需改用 `i18n.t()`） |

**注意**: `tool-accordion.tsx` 的折叠按钮和 `shell/index.tsx` 的 checkbox 按钮保留原生，因为它们有特殊的 DOM 结构需求（内嵌 SVG、checked 状态等）。但应在代码中加注释说明保留原因。

**验证**: 全局搜索 `<button` 只剩合理保留项 + 有注释说明

---

## Phase 2: 功能完整性修复（P0-P1）

### T3. SourcesTab 恢复来源渲染

**问题**: 来源列表核心渲染逻辑被注释，用户看到 source_count 但看不到来源内容
**文件**: `src/pages/sessions/sources-tab.tsx:34-44`

**方案**: 取消注释 `detail.sources.map(...)` 渲染块，改为：

```tsx
{detail.sources.map((src) => (
  <div key={src.source_id} className="rounded-sm border border-border px-2 py-1.5">
    <span className="font-mono text-[13px] text-muted-foreground">
      {src.path}
    </span>
  </div>
))}
```

同时补充 error 状态：

```tsx
const { data: detail, error } = useQuery({...});

if (error) {
  return <p className="text-[13px] text-destructive">{t("sources.error")}</p>;
}
```

**验证**: 选中任一 session，Sources 区域展示来源路径列表

---

### T4. 移除 UUID 调试信息

**问题**: 3 处把 message UUID 直接展示给用户
**涉及文件**:

| 文件 | 行号 |
|------|------|
| `src/pages/sessions/ai-bubble.tsx` | 35-37 |
| `src/pages/sessions/user-bubble.tsx` | 28-29 |
| `src/pages/sessions/tool-accordion.tsx` | 180-182 |

**方案**: 直接删除这 3 处 `<span>` 及其内容。

**验证**: 全局搜索 `uuid:` 在 TSX 文件中无结果

---

### T5. Shell tab 按钮迁移到 pill-tab class

**问题**: `shell/index.tsx` 的导航切换按钮用 inline className 复制了 `.pill-tab` 的样式
**文件**: `src/app/shell/index.tsx:61-74`（导航切换）、`session-detail-pane.tsx:161-170`（Feed/Graph toggle）

**方案**: `shell/index.tsx` 的 section 切换按钮改用 `pill-tab` / `pill-tab-active` / `pill-tab-idle` class：

```tsx
<button
  type="button"
  key={s}
  onClick={() => setSection(s)}
  className={`pill-tab ${section === s ? "pill-tab-active" : "pill-tab-idle"}`}
>
```

**验证**: 导航切换视觉与 Feed/Graph toggle 一致；`pill-tab` 相关样式定义在 `index.css` 中不被重复

---

## Phase 3: Graph 视图修正（P1）

### T6. Graph 容器高度改为 flex 适配

**问题**: Graph 容器硬编码 `h-[600px]`，不随面板大小变化
**文件**: `src/pages/sessions/session-detail-pane.tsx:175`

**方案**: 将条件 className 改为 `h-full`，并在外层 section 上确保 flex 布局撑满：

```tsx
// session-detail-pane.tsx 的 section 改为:
<section className="surface-card flex min-h-0 flex-1 flex-col overflow-hidden p-1">
  ...
  <div className={viewMode === "graph" ? "min-h-0 flex-1" : ""}>
```

同时 `session-graph.tsx` 的最外层 div 改为 `height: "100%"` 适配 flex 容器。

**验证**: 调整窗口大小，Graph 区域跟随面板高度变化

---

### T7. Graph 节点数硬上限

**问题**: main_path 过滤后仍可能产生数千节点，dagre 阻塞主线程
**文件**: `src/pages/sessions/session-graph.tsx:117-120`

**方案**: 加节点数上限，超出时截断并提示：

```tsx
const MAX_GRAPH_NODES = 300;
let mainMessages = transcript.messages.filter((m) => mainSet.has(m.id));
let truncated = false;
if (mainMessages.length > MAX_GRAPH_NODES) {
  mainMessages = mainMessages.slice(0, MAX_GRAPH_NODES);
  truncated = true;
}
```

在 Graph 区域顶部显示提示条（如有截断）：

```tsx
{truncated && (
  <div className="px-3 py-1.5 text-[12px] text-muted-foreground bg-secondary/50 rounded-md mb-2">
    {t("graph.truncated", { max: MAX_GRAPH_NODES, total: transcript.main_path.length })}
  </div>
)}
```

对应 i18n key 补充到 `en.json` / `zh-CN.json`。

**验证**: 选中一个长 session，Graph 正常渲染，超限时显示截断提示

---

### T8. Graph fitView 改用 onInit

**问题**: `setTimeout(() => fitView(), 100)` 是不可靠的布局同步方式
**文件**: `src/pages/sessions/session-graph.tsx:46-49`

**方案**: 删除 `useEffect` + `setTimeout`，改为 React Flow 的 `onInit` 回调：

```tsx
const onInit = useCallback((instance: ReactFlowInstance) => {
  instance.fitView({ padding: 0.15 });
}, []);
```

在 `<ReactFlow onInit={onInit} ...>` 中传入。

**验证**: 切换到 Graph 视图，节点自动 fit，无闪烁或延迟

---

## Phase 4: 代码质量收敛（P2）

### T9. SessionsPage 抽取行为 hooks

**问题**: 搜索、分组、选择、键盘导航逻辑耦合在一个组件中
**文件**: `src/app/shell/index.tsx` 中 `SessionsPage` 函数

**方案**: 分三步抽取，每步独立可验证：

1. **`useGroupedSessions(sessions, isSearching)`** — 提取分组聚合 + 折叠逻辑（行 181-214）
2. **`useSessionSelection(sessions, grouped, collapsedProjects)`** — 提取选择状态 + 自动选中首项 + 扁平 ID 列表（行 117-265）
3. **`useKeyboardNavigation(flatSessionIds, selectedId, onSelect)`** — 提取键盘事件处理（行 280-310）

hooks 放在 `src/app/shell/` 目录下，`SessionsPage` 只保留渲染 + 事件绑定。

**验证**:
- 每个 hook 可独立测试
- `SessionsPage` 的 JSX 不变，行为不变
- `tsc -b --noEmit` 通过

---

### T10. dangerouslySetInnerHTML 替换

**问题**: 2 处用 `dangerouslySetInnerHTML` 渲染简单计数文案
**文件**: `src/app/shell/index.tsx:426`（全选提示）、`540`（已选数量）

**方案**: 改为普通文本插值 + 强调数字用 `<span>` 包裹：

```tsx
// 之前
<span dangerouslySetInnerHTML={{ __html: t("sessions.selectAll", { count: sessions.length }) }} />

// 之后
<span className="text-[13px] text-muted-foreground">
  {t("sessions.selectAllPrefix")}
  <span className="font-medium text-foreground">{sessions.length}</span>
  {t("sessions.selectAllSuffix")}
</span>
```

对应调整 i18n key（拆分为 prefix + suffix，不再依赖 HTML 标签）。

**验证**: 全局搜索 `dangerouslySetInnerHTML` 在 TSX 文件中无结果

---

### T11. Graph i18n 补全

**问题**: Graph 层有 ~10 个硬编码英文字符串
**涉及文件**: `graph/nodes.tsx`、`graph/node-detail-panel.tsx`、`graph/build-tree.ts`

**方案**:

`nodes.tsx` — 三个组件的标签改为 `t()`:
- "User" → `t("graph.nodeUser")`
- "Assistant" → `t("graph.nodeAssistant")`
- "result" → `t("graph.nodeResult")`

`node-detail-panel.tsx` — roleLabel 的硬编码改为 i18n key。

`build-tree.ts` — label 中的硬编码字符串改为接收 `t` 函数参数，或在调用侧做映射。考虑到 `buildTree` 是纯数据函数，建议 label 保持英文，在节点渲染时再做 i18n 映射。

i18n keys 补充到 `en.json` / `zh-CN.json`。

**验证**: 切换到中文，Graph 节点标签显示中文

---

### T12. Graph inline styles 收敛

**问题**: `nodes.tsx`、`node-detail-panel.tsx` 大量 `style={{...}}`
**文件**: `src/pages/sessions/graph/nodes.tsx`、`src/pages/sessions/graph/node-detail-panel.tsx`

**方案**: 不是消灭所有 inline style，而是把可复用的视觉 token 集中化：

1. 节点尺寸常量 `W=200` 已有，保留
2. 节点通用样式提取为 CSS class（在 `index.css` 或 `graph/nodes.css` 中）：

```css
.graph-node {
  @apply rounded-md border border-border bg-card text-foreground;
}
.graph-node-label {
  @apply text-[11px] leading-[1.35] text-foreground;
}
.graph-node-tag {
  @apply text-[9px] font-semibold uppercase tracking-[0.8px];
}
```

3. 颜色相关的 inline style（如 `toolColor`）保留，因为它们是动态计算的

**验证**: 节点视觉不变，但通用样式通过 CSS class 而非 inline style 控制

---

## Phase 5: 工程化建设（P3）

### T13. 魔法数字提取

**涉及文件**: `session-row.tsx:62`（`title.length > 80`）、`session-graph.tsx`（`MAX_GRAPH_NODES`）、`transcript-view.tsx`（`visibleCount` 初始值 100、增量 80）

**方案**: 在 `src/lib/constants.ts` 中集中定义有语义的阈值：

```ts
export const TITLE_TRUNCATE_LEN = 80;
export const GRAPH_MAX_NODES = 300;
export const TRANSCRIPT_INITIAL_COUNT = 100;
export const TRANSCRIPT_LOAD_MORE = 80;
```

不提取无语义的数字（如 padding、gap 等）。

**验证**: 搜索 `> 80`、`100`、`80` 等在相关文件中被常量替代

---

### T14. 补充高价值测试

**范围**: 不追求覆盖率，只补高价值测试

优先级：
1. `src/lib/formatters.ts` — 时间格式化、项目标签格式化
2. `src/pages/sessions/graph/build-tree.ts` — 节点过滤、re-parent、树构建
3. `src/pages/sessions/transcript-view.tsx` 的 `groupMessages()` — 消息分组逻辑

**验证**: `vitest run` 通过

---

## 执行依赖关系

```
T1 ─┐
T2 ─┤
T3 ─┼── 可并行，无依赖
T4 ─┤
T5 ─┘

T6 ─┐
T7 ─┼── 依赖 T2（Graph 按钮）, 可并行
T8 ─┤
T11─┤
T12─┘

T9 ──── 独立，可与 Phase 3 并行
T10──── 独立

T13──── 依赖 T9（重构后提取常量）
T14──── 随时可补
```

## 总览

| Phase | 任务数 | 预估工作量 | 风险 |
|-------|--------|-----------|------|
| Phase 1 (P0) | T1-T5 | 小 | 低 |
| Phase 2 (P0-P1) | T3-T5 | 小 | 低 |
| Phase 3 (P1) | T6-T8, T11-T12 | 中 | 中（Graph 改动） |
| Phase 4 (P2) | T9-T10 | 中 | 低 |
| Phase 5 (P3) | T13-T14 | 小 | 无 |

建议执行顺序：Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5。Phase 内任务可并行。

---

*基于 `docs/reviews/2026-04-19-frontend-review-spec.md` 生成 · 2026-04-19*
