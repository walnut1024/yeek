# End-to-End Tests

E2E tests depend on real infrastructure (network, API keys, file system).
They are NOT run as part of `cargo test`.

## Running

```bash
# Requires DEEPSEEK_API_KEY or equivalent in environment
cargo xtask e2e

# Or manually:
cargo test -p vendor-proxy -- --ignored -- e2e
```

## Conventions

- Test files: `e2e/*.rs` or `vendor_proxy/tests/*.rs` (marked `#[ignore]`)
- Each test must be self-contained and clean up after itself
- Use unique ports (see `PORT_COUNTER` pattern in `vendor_proxy/tests/`)
- Skip gracefully if required env vars are missing
