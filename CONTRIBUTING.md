# Contributing

本文档定义本仓库 Rust 相关代码的开发约定。目标是保证正确性、一致性、可维护性与可测试性。

## 0. 规范执行

本规范通过以下机制强制执行：

| 层级 | 工具 | 触发时机 |
|------|------|----------|
| 编辑器 | `.editorconfig` | 保存时自动应用 |
| 提交前 | `scripts/pre-commit` | `git commit` 前自动运行 |
| PR 门禁 | `.github/workflows/ci.yml` | 每次 push / PR |
| 本地检查 | `cargo xtask ci` | 手动运行全量检查 |

### 安装 pre-commit hook

```bash
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

提交前会自动运行 `cargo fmt --check` 和 `cargo clippy`。

### 本地完整检查

```bash
cargo xtask ci          # fmt + clippy + test + audit
cargo xtask fmt         # 仅格式化检查
cargo xtask lint        # 仅 clippy
cargo xtask test        # 仅测试
```

## 1. 原则

- 正确性优先，其次可读性、可维护性、性能。
- 先写清晰代码，再做抽象；先有数据，再做优化。
- 所有实现应便于测试、审查和重构。

## 2. 目录约定

- `src/lib.rs`：核心业务与可复用逻辑
- `src/main.rs`：薄入口，只做启动与装配
- `src/bin/`：多个二进制入口
- `tests/`：集成测试，由 `cargo test` 自动运行
- `e2e/`：端到端测试，依赖真实基础设施，不纳入 `cargo test`
- `xtask/`：工程自动化任务
- `docs/`：设计文档、ADR、接口说明

推荐结构：

```text
src/
  lib.rs
  main.rs
  bin/

tests/
  integration/
  api/
  db/
  fixtures/

e2e/
xtask/
docs/
```

## 3. 模块设计

- 单一职责，避免超大模块和混合分层。
- 优先 `pub(crate)`，谨慎暴露 `pub`。
- 业务、存储、配置、适配层边界清晰。
- 禁止跨层穿透和隐式耦合。
- 公共接口保持最小且稳定。

## 4. 命名规范

- 文件、模块、函数：`snake_case`
- 类型、trait、enum：`PascalCase`
- 常量、静态变量：`SCREAMING_SNAKE_CASE`
- 测试命名：`test_<函数>_<场景>_<预期>`

## 5. 代码风格

- 强制使用 `rustfmt`
- 强制通过 `clippy`
- 新增代码不得引入 warning
- 函数应短小、意图单一
- 注释解释“为什么”，不重复“做了什么”
- 避免过度抽象、过度泛型化、过度宏化

## 6. 错误处理

- 业务路径禁止滥用 `unwrap()` / `expect()`
- 库接口统一返回 `Result<T, E>`
- 错误类型必须有语义
- 错误信息必须带上下文
- 禁止吞错、忽略错误

## 7. 类型与所有权

- 优先用类型系统表达约束
- 能借用就借用，避免无意义 `clone()`
- 公共接口避免暴露复杂生命周期
- 在零成本抽象和可读性之间，优先可读性

## 8. 并发与异步

- 仅在必要时引入 async
- 明确超时、取消、重试、资源释放策略
- 优先消息传递，最小化共享状态
- 禁止跨 `await` 持锁

## 9. 测试规范

- 单元测试覆盖核心逻辑、边界条件、错误分支
- 核心 `pub(crate)` 逻辑必须覆盖
- 集成测试放在 `tests/`，由 Cargo 自动运行
- 端到端测试放在 `e2e/`，由 `cargo xtask e2e` 驱动
- 外部依赖优先使用 mock 或 in-memory
- 修复缺陷必须补回归测试
- 测试应稳定、可重复、无顺序依赖

## 10. 自动化与 `cargo xtask`

- `cargo test`：运行单元测试与集成测试
- `cargo xtask e2e`：运行端到端测试
- `cargo xtask ci`：统一本地 CI 流程
- `cargo xtask` 仅用于工程任务编排，不承载业务逻辑

建议统一入口：

```bash
cargo xtask fmt
cargo xtask lint
cargo xtask test
cargo xtask e2e
cargo xtask ci
```

## 11. 依赖管理

- `Cargo.toml` 默认设置：

```toml
publish = false
```

- 禁止 `*` 版本
- 禁止 `git` 依赖，内部 fork 除外
- 优先使用 workspace 统一依赖版本
- 仅启用必要 feature，避免默认 feature 膨胀
- 升级依赖前必须通过完整 CI

## 12. 配置与环境

- 配置与代码分离
- 敏感信息不得提交到仓库
- 配置项必须有默认值、校验和说明
- 环境差异必须显式管理

## 13. 日志与可观测性

- 使用结构化日志
- 记录关键上下文：请求 ID、资源 ID、耗时、错误原因
- 禁止输出敏感信息
- 关键路径应具备 tracing 和 metrics 能力

## 14. 文档要求

- 公共 API 必须有文档注释
- 复杂模块必须补设计说明
- README 至少包含：目标、运行方式、测试方式、目录结构
- 关键设计决策建议记录 ADR

## 15. CI 基线

至少执行：

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

如已接入 `xtask`，统一使用：

```bash
cargo xtask ci
```

## 16. 代码审查

- PR 保持单一目的、小步提交
- Review 重点关注：正确性、边界条件、复杂度、接口稳定性、测试覆盖
- 复杂实现必须说明设计理由
- 未验证的优化和抽象不接受

## 17. 安全

- 默认不信任外部输入
- 输入必须校验，输出必须约束
- 文件、网络、命令执行等高风险操作必须受控
- 定期进行依赖漏洞扫描
