# AGENTS.md

## Cursor Cloud specific instructions

This is a Rust library crate (`queued-task`) for concurrent queue-based task processing. No external services or databases are required.

### Key commands

| Task | Command |
|------|---------|
| Build | `cargo build` |
| Test | `cargo test` |
| Lint | `cargo clippy -- -D warnings` |
| Format check | `cargo fmt --check` |
| Format fix | `cargo fmt` |

### Notes

- Minimum Rust version: 1.70 (current toolchain 1.83.0 satisfies this).
- The test in `src/lib.rs` spawns 20 concurrent tasks with a 1-second sleep each, so `cargo test` takes ~10 seconds to complete.
- Doc-tests run the example from `README.MD`, which also exercises the library end-to-end.
