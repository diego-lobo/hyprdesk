## What this changes

<!-- One or two sentences. What is different afterwards? -->

## Why

<!-- The problem being solved. Link an issue if there is one. -->

## How it was verified

<!--
Say what you actually ran, including live-session testing if the change
touches compositor behavior. "Not verified live" is a fine answer, just
say so rather than leaving it implied.
-->

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] Tried against a live Hyprland session

## Checklist

- [ ] No `unsafe`, and no new dependency on Hyprland internals
- [ ] New compositor actions go through `hypr::Command`, not `format!` at
      the call site
- [ ] Pure logic has tests
- [ ] Docs updated if behavior changed
