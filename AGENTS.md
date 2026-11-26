# Notes for AI agents

Read this before changing CargoFrame. It holds the rules a change must keep; how to make common changes is in
[docs/customizing.md](docs/customizing.md), where things live in [docs/development-plan.md](docs/development-plan.md#5-architecture),
and why things are as they are in [docs/decisions.md](docs/decisions.md).

## The gate

```sh
CARGO_INCREMENTAL=0 scripts/dev/check.sh quick   # while working
CARGO_INCREMENTAL=0 scripts/dev/check.sh all     # before every push (UI build, budgets, overflow check, end to end)
```

Run one at a time. CI repeats it, on Windows too.

## Rules a change must keep

- **Nothing is sent without the person pressing a button.** No telemetry.
- **Secrets only in the credential store** (`app/src/secrets.rs`, `ctx.secret`); never in a settings file, a log, an
  error message or a URL.
- **Requests only through `core/net`** (HTTPS, checked redirects, size cap). Text from a screen, a file or another
  program never reaches a command line.
- **A new command** goes in `app/build.rs` and is granted only to the window that needs it (`app/capabilities/`).
- **Deletions** only through `app/src/cleanup.rs`, inside CargoFrame's own folders.
- **Every text people read** goes through `t()` (UI) or `t`/`tf`/`msg!` (Rust); a text passed to `t()` through a
  variable is marked `msg!`/`msg`. Then `node scripts/i18n/check.mjs --missing`, translate into fr, es, it, de,
  `--merge`, `--prune`. Glossary: [docs/i18n.md](docs/i18n.md).
- **Types shared with the windows** come from Rust: `cargo test` writes `ui/src/lib/generated/`; commit those files
  with the change. A Rust list the window copies by hand gets a line in `ui/src/lib/mirrors.test.ts`.
- **A send is safe to repeat**: record what was done in `ctx.progress`.
- **Settings files are versioned**: a new field has a default, so older files keep working.
- **Windows stay inside their boxes** at the smallest size: a new state gets a view in `ui/scripts/overflow-check.mjs`.
- `unsafe` is forbidden outside the platform crates.

## What tests cannot see

Real desktops (Windows, GNOME, KDE, Wayland), real accounts, real certificates. Say in the pull request what needs one,
and update [docs/status.md](docs/status.md).

## Docs to keep current

A user-visible change: `README.md`, `docs/design.md`, `CHANGELOG.md` (Next). A decision: `docs/decisions.md`. What is
verified: `docs/status.md`.
