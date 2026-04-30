## Summary

<!-- Brief description of the change and why -->

## Checklist

### Code Quality
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No new `.unwrap()` in business logic (use `.expect()` with a message or `Result` propagation)
- [ ] Public API changes have `///` documentation
- [ ] New `pub fn` is justified (prefer `pub(crate)`)

### Testing
- [ ] `cargo test --workspace` passes
- [ ] New functionality has tests (unit for `pub(crate)`, integration for cross-module)
- [ ] Bug fixes include a regression test

### Design
- [ ] No cross-module coupling (store ↔ adapter ↔ commands boundaries respected)
- [ ] New dependencies are justified and use minimal features
- [ ] Complex changes have an ADR in `docs/adr/`

### Security (if applicable)
- [ ] External input is validated (see `is_valid_uuid`, `shell_quote`)
- [ ] No sensitive data in logs
- [ ] `cargo audit` passes
