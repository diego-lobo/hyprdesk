# Contributing to hyprdesk

Thanks for taking a look. Issues, questions, and pull requests are all
welcome, including "this broke on my setup" reports with no diagnosis
attached.

## Getting set up

```bash
git clone https://github.com/diego-lobo/hyprdesk
cd hyprdesk
cargo build
cargo test
```

You do not need Hyprland running to build or to run the test suite: every
test covers pure logic (the desk arithmetic, command rendering, the
protocol, the window-memory state machine) with no compositor in the loop.
That is deliberate, and new logic should keep that property.

To try your build against a live session:

```bash
cargo install --path . --root ~/.local --force
hyprctl reload && hyprctl configerrors    # must come back clean
hyprdesk status
```

## Before you open a pull request

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs exactly these three. Clippy is on `pedantic` and warnings are
errors, so it is worth running locally first.

## Code standards

The bar is code a senior engineer would sign off on without comments.
Concretely:

- **No `unsafe`.** It is `forbid`den at the crate level and that is not
  negotiable.
- **Typed errors, no stringly-typed protocol surfaces.** Anything crossing
  a boundary (the compositor, the control socket, the command line) gets a
  real type. The `hypr::Command` enum exists because the previous
  `format!`-built dispatch strings silently changed meaning under a
  Hyprland parser change and nothing caught it.
- **Document invariants, not mechanics.** Comments should say *why* the
  code is shaped this way, especially where it encodes a compositor
  behavior discovered the hard way. Several of the stranger-looking guards
  in `daemon.rs` are like this, and each one names the bug it prevents.
- **Tests for pure logic.** If a function can be tested without a
  compositor, test it.
- **Readability first.** Clever beats verbose only when it is also
  clearer.

Style notes: `cargo fmt` settles formatting arguments. Prose in code,
comments, docs, and commit messages uses plain ASCII punctuation, so a
hyphen rather than an em dash.

## Architecture in one minute

| File | Role |
|------|------|
| `src/model.rs` | The desk arithmetic. Pure, no I/O |
| `src/hypr.rs` | The only module that talks to Hyprland |
| `src/protocol.rs` | The client/daemon wire format |
| `src/daemon.rs` | The resident state owner and event loop |
| `src/client.rs` | Sends one request, prints the reply |
| `src/waybar.rs` | Renders the waybar status stream |
| `src/error.rs` | The crate-wide error type |

The daemon is single-threaded where state is concerned: two producer
threads feed one channel, and the main loop is the only place state is
read or written, so there is no locking anywhere. Keep it that way.

[`docs/DESIGN.md`](docs/DESIGN.md) has the full reasoning, including why
this is an IPC client rather than a compositor plugin.

## Things worth knowing before you change compositor behavior

hyprdesk drives Hyprland exclusively through its **stable public IPC**.
Adding a dependency on compositor internals defeats the entire reason the
project exists, so pull requests that do that will not be merged.

If you need a new compositor action, add a variant to `hypr::Command`
rather than building a string at the call site, and add its rendering to
the test in `src/hypr.rs` so the exact wire form is pinned.

## Reporting bugs

Please include your Hyprland version (`hyprctl version`), whether you are
on Omarchy, your monitor layout (`hyprctl monitors -j`), and what
`hyprdesk status` says. The issue template asks for these.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
