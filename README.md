# CargoFrame

"CargoFrame" is a computer local context sharing application that send data in a user defined web destination.

A tray app for Windows 11 and Linux. A shortcut captures the program in focus, finds its project
folder, reads the data you asked for, and opens a window where you review it and share it to Jira,
Google, Reddit, Claude or any API you describe. Nothing leaves your computer until you click.

> **Status: early.** The plan is written and the dependency-free core logic is built and tested.
> The tray app, installers, capture, window builder and connectors are not built yet. See
> [`docs/status.md`](docs/status.md).

## Layout

| Path | Contents |
|------|----------|
| `core/context` | Project-folder climb, `{{variable}}` templates, read-only allow-list |
| `core/safe` | Retry with backoff, failure classes, fallback ladder, zip naming |
| `core/layout` | Window layout model, grid, validation chips |
| `core/handoff` | Claude chat links, clipboard plan, pinned and recent chats |
| `docs/` | Decisions, signing, spike runbook, status |
| `scripts/` | Helper scripts (signing material) |
| `installers/` | Installer notes (built in Phase 1) |
| `.github/` | CI workflow and Dependabot config |

Planned: `core/{config,rules,capture,bundle,connectors}`, `app/` (Tauri shell) and `ui/` (Svelte).

## Develop

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Rust stable (see `rust-toolchain.toml`).

## Documents

- [Decisions](docs/decisions.md): what was decided, and how the project brief changed the plan
- [Status](docs/status.md): what works, what is verified, what is blocked
- [Spikes](docs/spikes.md): the experiments to run before building the shell
- [Signing](docs/signing.md): certificates and update keys (never commit these)
