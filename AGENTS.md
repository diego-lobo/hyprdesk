# Agent and contributor guide - hyprdesk

Conventions for anyone working in this repo, human or AI. Read
[`docs/DESIGN.md`](docs/DESIGN.md) before changing behavior and
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the build and review workflow.

## What this project is

A resident daemon plus CLI that gives Hyprland shared-monitor virtual
desktops ("desks"), driven **exclusively over Hyprland's stable public
IPC**. It is deliberately not a compositor plugin. That boundary is the
whole point of the project, so proposals that cross it are out of scope.

## Hard constraints

- **No `unsafe`.** `unsafe_code = "forbid"` at the crate level.
- **No sudo, ever.** Not in the install path, not at runtime. hyprdesk
  exists because the plugin route's root-owned `hyprpm` state kept
  failing. Everything must build, install, load, and update from the
  user's home directory with user permissions only.
- **No stringly-typed protocol surfaces.** Compositor actions go through
  the `hypr::Command` enum, wire messages through `protocol::Request`.
  A `format!`-built dispatch string is how the Hyprland 0.56 parser change
  slipped through undetected once already.
- **No dependency on Hyprland internals**, headers, or plugin ABI.
- **Never modify anything under `/usr/share/omarchy/`.** It is
  package-owned and edits are lost on update. Reading it is encouraged and
  is the best source of truth for Omarchy's Lua and Quickshell contracts.
- **Plain ASCII punctuation** in code, comments, docs, and commit
  messages. A hyphen, not an em dash.
- **Commits carry no trailers or footers.** No `Co-Authored-By`, no tool
  attribution.
- After any Hyprland config change, run `hyprctl reload` then
  `hyprctl configerrors` and fix until clean.
- **Do not self-verify visual outcomes with headless screenshots.** Ask a
  human to look at the bar or the screen.

## Module map

| File | Role | Notes |
|------|------|-------|
| `src/model.rs` | Desk arithmetic | Pure, no I/O, fully unit tested |
| `src/hypr.rs` | The only module that talks to the compositor | Owns `Command` and the Lua rendering |
| `src/protocol.rs` | Client/daemon wire format | Typed both directions |
| `src/daemon.rs` | Resident state owner, event loop | Two producer threads, one channel, no locks |
| `src/client.rs` | Sends one request, prints the reply | |
| `src/waybar.rs` | Waybar custom-module rendering | For non-Omarchy setups |
| `src/error.rs` | Crate-wide typed error | |
| `config/hypr/hyprdesk.lua` | Keybinds and daemon autostart | Symlinked to `~/.config/hypr/` |
| `config/omarchy/plugins/hyprdesk.desks/` | Quickshell bar widget | Symlinked to `~/.config/omarchy/plugins/` |

The daemon holds only `current` and `last` desk. Everything else is
queried live from the compositor at the moment it is needed, so it can
never act on a stale picture. Preserve that.

## System integration is symlinked from `config/`

`~/.config/hypr/hyprdesk.lua` and
`~/.config/omarchy/plugins/hyprdesk.desks` are **symlinks** into this
repo's `config/` tree. Edit the repo copies. Never replace a symlink with
a regular file, and never edit through the symlink target path as if it
were a separate file.

## Hyprland 0.56 specifics worth not rediscovering

- The config engine is **Lua**. The legacy text grammar is gone: `keyword`
  is rejected outright ("Use eval."), `dispatch` arguments parse as Lua,
  and `[[BATCH]]` is retired. Drive the compositor with `eval <lua chunk>`.
  The verified grammar table is in `docs/DESIGN.md`.
- `hyprctl eval` returns only `ok`, never the expression's value. To
  introspect the `hl` API, have the chunk write results to a file with
  `io.open(...)` and read the file.
- Overriding an Omarchy default bind needs `hl.unbind(...)` first, since
  the defaults load before user modules. The number row is keycode-based:
  `code:10`..`code:19` are keys 1..0.
- `hyprctl binds` does **not** populate `key`/`keycode` for these binds,
  and dispatchers appear as opaque `__lua` handles. Match on
  `description` instead. Stock binds report identically, so an empty
  grep here is not evidence of a problem.
- A config reload **wipes** eval-registered workspace rules, which is why
  the daemon re-asserts them on `configreloaded`.

## Verifying a change

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Against a live session:

```bash
cargo install --path . --root ~/.local --force
hyprctl reload && hyprctl configerrors     # must be clean
hyprdesk status
```

The four integration points to re-check after any Hyprland or Omarchy
update are listed at the end of [`docs/DESIGN.md`](docs/DESIGN.md). The
daemon's own logic has never broken on an update; the edges have.

## Working style

- Settle architecture, library, and design picks on the merits. Do not
  defer them automatically.
- **Never contrive.** If an approach fails, report the finding. Do not
  paper over it with a workaround that hides the failure.
- Prefer extending the typed surfaces over adding special cases at call
  sites.

## Reference material

`reference/hyprland-virtual-desktops/` (gitignored) is the upstream plugin
at commit `70a1ae6c`, the exact behavioral reference hyprdesk emulates.
Re-clone it with `scripts/fetch-reference.sh`. Its extracted semantics are
recorded durably in [`docs/UPSTREAM-SEMANTICS.md`](docs/UPSTREAM-SEMANTICS.md),
so read that first; go to the source only for detail it does not cover.
