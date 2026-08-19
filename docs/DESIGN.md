# DESIGN - hyprdesk

Status: IMPLEMENTED and RUNNING. Decided 2026-07-26; ported to Omarchy 4
"Quattro" / Hyprland 0.56.2 on 2026-08-18 (see the port section below).
Architecture B (IPC-based emulation) in Rust:
a resident daemon plus thin CLI client, single binary. Rationale: the plugin
route (A) couples to internal C++ headers that drift every release - the
exact failure class this project escapes - and a Rust in-process plugin
would additionally need a C++ shim carrying that same burden (Hyprland's
plugin ABI passes std::string/SP<> across the boundary; no C ABI exists).
The IPC surface is the stable public contract. Known trade-off: batched
multi-monitor switches are not compositor-atomic (possible one-frame
stagger); to be measured in practice - if it proves visibly bad, that
finding reopens route A.

Upstream behavior reference extracted to `docs/UPSTREAM-SEMANTICS.md`.

## Product behavior (what "done" looks like)

A "desk" is one logical desktop spanning ALL connected monitors. Switching
to desk N changes what EVERY monitor shows, atomically enough that it feels
like one action (macOS Spaces / Windows virtual desktops / KDE Activities
feel). Behaviors to provide, matching the upstream plugin's dispatcher
surface (extract exact semantics from `reference/` source, not memory):

- `vdesk N` - switch every monitor to desk N.
- `movetodesk N` - move active window to desk N (and follow).
- `movetodesksilent N` - move active window to desk N (stay).
- `nextdesk` / `prevdesk` - cycle desks.
- `backandforth` - jump to the previously active desk.
- Graceful single-monitor degradation (undocked laptop = plain workspaces).
- Monitor hotplug: dock/undock re-welds desks without losing windows.
- Nice-to-have from upstream: sticky apps (window always on current desk).

## Architecture candidates

### A. C++ Hyprland plugin (fork of / from-scratch like upstream)

- We PROVED the toolchain works: commit `70a1ae6c` builds against system
  headers, no sudo (see `docs/BACKGROUND.md`). A fork would build the same
  way, loaded via a `plugin = /path/to/virtual-desktops.so` config line.
- Cost: couples to Hyprland's INTERNAL headers, which drift every release
  (the v0.55.4-pin commit already needs a different include layout than the
  v0.55.3-pin commit). Every `pacman -Syu` that bumps Hyprland risks a
  rebuild-or-patch session. This is the exact hassle class we are escaping,
  now with us as the sole maintainer.
- Upside: true compositor-internal integration - perfectly atomic switches,
  first-class per-desk state.

### B. IPC-based emulation (RECOMMENDED)

Drive Hyprland exclusively through its STABLE public surface: request
socket actions, workspace rules, and the socket2 event stream. No
compositor internals, so Hyprland updates cannot break the BUILD - the
IPC surface is the compatibility contract. (Hyprland 0.56 did change the
action GRAMMAR on that surface; see "Omarchy 4 / Hyprland 0.56 port"
below. The port was a one-file change with no architectural impact,
which is the trade the architecture was chosen to make.)

Sketch:
- **Mapping:** desk `d` = workspace `d` on monitor 1, workspace `d+10` on
  monitor 2 (`d+20` on a third, etc.). Stable under hotplug, human-legible
  in waybar (desk identity = last digit), and workspace rules
  (`workspace = N, monitor:X`) pin each workspace to its monitor so plain
  `dispatch workspace N` lands correctly.
- **Switch desk d:** one `hyprctl --batch` of per-monitor
  `focusmonitor` + `workspace` dispatches (or rule-pinned `workspace`
  dispatches), ending focus on the monitor the cursor is on.
- **movetodesk / silent:** `movetoworkspace(silent)` targeting the desk's
  workspace for the active window's monitor.
- **backandforth / next / prev:** trivial state (a file or daemon variable).
- **Hotplug:** subscribe to socket2 `monitoradded`/`monitorremoved`,
  regenerate workspace->monitor rules, re-distribute live workspaces.
- **Waybar:** full custom control BONUS - a custom module (or workspace
  format mapping) can show the DESK number cleanly, fixing the "desk 2
  highlights ws 3+4" cosmetic wart the plugin route would have had.
  IMPLEMENTED 2026-07-26: `hyprdesk waybar` streams custom-module JSON
  (protocol `Request::Subscribe(StreamFormat::Waybar)`, rendering in
  `src/waybar.rs`); `~/.config/waybar/config.jsonc` replaces
  `hyprland/workspaces` with `custom/hyprdesk`. Occupancy is desk-level
  (a desk is occupied if ANY of its workspaces has windows, across all
  monitors - aggregation waybar's own module cannot do). Visuals mirror
  stock Omarchy: desks 1-5 persistent (dimmed via pango alpha when
  empty), 6-10 appear only while occupied, active desk is the stock dot
  glyph. Scroll on the module cycles desks; per-desk click targets are
  impossible in a single custom module (accepted).
  SUPERSEDED on Omarchy 2026-08-18: Omarchy 4 has no waybar. Its bar is
  now a Quickshell plugin host and the desk strip is a user bar-widget,
  `~/.config/omarchy/plugins/hyprdesk.desks/` (see the port section
  below). The `waybar` subcommand and `src/waybar.rs` remain the supported
  path for plain-Hyprland setups that still run waybar, and are the
  daemon's only push-notification surface.

Known caveat to evaluate honestly: a batched multi-dispatch switch is not
compositor-atomic; there may be a visible one-frame stagger between
monitors. Measure it in practice before declaring it acceptable (upstream
plugin A does not have this issue).

### Sub-decisions for B (decided 2026-07-26)

- **Daemon + CLI client in one binary** (`hyprdesk daemon` vs subcommands).
  Daemon holds desk state (current/last), listens on socket2 for
  hotplug/config events, serves clients on its own control socket at
  `$XDG_RUNTIME_DIR/hyprdesk/$HYPRLAND_INSTANCE_SIGNATURE.sock`. Hyprland
  sockets: `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket{,2}.sock`.
- **Language: Rust.** std UNIX sockets for all IPC; serde_json for parsing
  `hyprctl -j`-equivalent query replies (hand-rolled JSON parsing is not
  worth the brittleness).
- **Monitor index** = position in the monitor list sorted by Hyprland
  monitor id (eDP-1 is id 0 on this machine; externals enumerate after).
  Desk `d` (1..=10) on monitor index `m` = workspace `d + 10*m`. Deviation
  from upstream's consecutive-block mapping, on purpose: workspace ids keep
  their desk meaning across monitor-count changes (no layout memory needed),
  waybar stays legible (last digit = desk), and `workspace = N, monitor:X`
  rules can pin every workspace to its monitor.
- **Focus after switch:** upstream parity - the previously focused monitor
  is switched last so focus and cursor stay on it.
- **Same-desk re-press:** no-op in v1 (upstream swaps workspaces between two
  monitors; that fights the pinning rules; revisit as an option).
- **Undock (monitor removed):** windows on the removed monitor's workspace
  `d+10m` are merged into the surviving desk workspace `d` for every desk,
  so nothing becomes unreachable. REVISED 2026-07-29: redock now DOES
  un-merge, via window memory. Suspend (s2idle) drops the DP link, so
  Hyprland tears the external monitor down on every sleep and each wake
  looked like a fresh dock with all windows stranded on the laptop
  (verified in the Hyprland log: "Connector DP-2 disconnected" during
  suspend). The daemon remembers each evacuated window's home workspace
  at merge time and sends it back when the monitor slot returns; windows
  that closed or that the user re-placed while undocked are left alone
  and forgotten. Memory is keyed by monitor slot, not name, so it also
  covers a monitor re-enumerating under a new connector name. Upstream
  gets the same effect from per-monitor-set layout caches
  (`rememberlayout = monitors`) because it moves workspaces between
  monitors rather than windows between workspaces.
- **Desk tracking guards:** ADDED 2026-07-31 after a redock silently reset
  desk 8 to desk 1 (the reported symptom was shift+super+N needing two
  presses). Hyprland re-juggles workspaces around a monitor (un)plug and
  emits `workspacev2` BEFORE the monitor event, so the daemon was recording
  that juggling as user desk switches and the re-weld then applied the
  corrupted desk. A `workspacev2` is now only trusted when the monitor set
  matches the last successful re-weld AND every monitor shows the same desk
  on its own slot (`desk_shown_everywhere`); mid-juggle states always fail
  one of the two.
- **Address-pinned moves:** REVISED 2026-07-31. `movetodesk(silent)`
  resolves the active window's address up front and pins every dispatch to
  it. The unpinned "active window" dispatch form resolves the window
  mid-batch, after a cross-monitor `workspace` dispatch has already stolen
  focus, and was seen moving the wrong window (the stale focus pointer
  left after a re-weld). A follow ends with an explicit `focuswindow` so
  focus deterministically lands on the moved window.
- **Special/scratchpad workspace:** untouched; negative workspace ids are
  ignored by the daemon entirely.
- **SUPER+SHIFT+ALT+arrows** (`movecurrentworkspacetomonitor`): unbind at
  keybind-wiring time; it breaks the weld and has no desk-world meaning.
- **nextdesk/prevdesk:** bounded 1..=10; plain forms clamp, cycle forms
  wrap (upstream's unbounded desk creation does not fit a 10-key row).

## Omarchy 4 / Hyprland 0.56 port (2026-08-18)

Omarchy 4.0.0 "Quattro" (Hyprland 0.56.2) broke every integration point
at once. The daemon's LOGIC survived untouched; only the two edges that
speak to the outside world moved. Root causes, all verified live:

1. **Hyprland's config engine is now Lua, and the legacy text grammar is
   gone.** `keyword ...` on the request socket answers "keyword can't
   work with non-legacy parsers. Use eval.", and `dispatch workspace 4`
   is now parsed as the Lua expression `hl.dispatch(workspace 4)`, a
   syntax error. This killed the daemon at startup, on its very first
   action (asserting pinning rules). `[[BATCH]]` is likewise retired.
   Replacement: a single `eval <lua-chunk>` request. Because one chunk is
   one compositor-side execution with nothing interleaved, this is
   TIGHTER than the old batch, which parsed each item separately.
2. **Omarchy's Hyprland config moved from `.conf` to `.lua`.** The
   migration left the old `~/.config/hypr/*.conf` files on disk but
   Hyprland loads `hyprland.lua` and never reads them (confirmed in the
   compositor log: "Using lua config found at .../hyprland.lua"), so
   `hyprdesk.conf` was inert - no binds, no daemon autostart.
3. **Waybar is gone.** The bar is a Quickshell plugin host
   (`omarchy-shell`); its stock `omarchy.workspaces` widget filters to
   workspace ids 1-10, so under hyprdesk the external monitor's desk
   workspaces (11+) are invisible and NOTHING highlights whenever focus
   sits on that monitor.

### What changed

- **`src/hypr.rs`:** `batch(&[String])` became `eval(&[Command])`, where
  `Command` is a typed enum of the five compositor actions hyprdesk
  needs, each rendering to one `hl.*` Lua statement. This removed the
  last stringly-typed protocol surface in the crate - the old code built
  dispatch strings by `format!` at four call sites, which is exactly the
  construct that silently changed meaning under the new parser.
  Compositor-supplied values (monitor names, window addresses) are
  interpolated through `lua_quote`, so an odd name cannot be read as Lua.
- **Everything else in the daemon:** unchanged apart from the builders
  returning `Vec<Command>` instead of `Vec<String>`. Desk model, hotplug
  re-weld, window memory, tracking guards, protocol: all untouched.
- **`~/.config/hypr/hyprdesk.lua`** replaces `hyprdesk.conf`, required
  from `hyprland.lua` after the Omarchy defaults. Same bindings as
  before, expressed with `hl.unbind` + `o.bind` and a loop over the
  number row instead of 30 hand-written lines. Full revert is still one
  line plus one file.
- **`~/.config/omarchy/plugins/hyprdesk.desks/`** replaces the waybar
  module on Omarchy: a Quickshell bar widget rendering the same desk
  strip.

### Verified action grammar (Hyprland 0.56.2)

| Purpose | Lua statement |
|---------|---------------|
| switch a monitor's workspace | `hl.dispatch(hl.dsp.focus({ workspace = "15" }))` |
| focus a window | `hl.dispatch(hl.dsp.focus({ window = "address:0x..." }))` |
| move a window, no follow | `hl.dispatch(hl.dsp.window.move({ window = "address:0x...", workspace = "12", follow = false }))` |
| move a workspace to a monitor | `hl.dispatch(hl.dsp.workspace.move({ workspace = "12", monitor = "HDMI-A-1" }))` |
| pin a workspace to a monitor | `hl.workspace_rule({ workspace = 21, monitor = "eDP-1" })` |

Properties confirmed by probing the live compositor, all of which the
daemon depends on:

- `eval` accepts a multi-statement chunk; a raising statement aborts the
  rest and the reply carries the error text instead of `ok`, which keeps
  `eval`'s failure semantics identical to the old batch's.
- Re-registering `hl.workspace_rule` for a workspace REPLACES its rule
  (no duplicate accumulation across re-welds).
- A config reload WIPES eval-registered workspace rules, so the existing
  `configreloaded` re-assertion is still required. Verified still working
  after the port: 20 rules restored post-reload.
- `hyprctl binds` reports keycode binds with `key: ""` and `keycode: 0`
  for Lua-registered and stock binds alike, so it cannot be used to check
  which physical key a bind sits on. Match on `description` instead.

### Bar widget design

The widget derives the desk strip PURELY from compositor state that
Quickshell already tracks reactively (`Hyprland.workspaces`,
`Hyprland.focusedWorkspace`), applying the same `((id - 1) % 10) + 1`
mapping as `src/model.rs`. It does not read the daemon's socket; the
daemon is invoked only to ACT on a click or scroll. Rationale: the bar
stays correct across daemon restarts, needs no stream plumbing, and
cannot drift into a state the compositor disagrees with. Visuals mirror
the previous waybar strip (persistent 1-5, higher desks only while
occupied, stock dot glyph for active, desk 10 labelled "0"). Bonus over
waybar: per-desk click targets are now possible, which the single custom
module could not do.

## Keybind plan

All overrides go in ONE new file (`~/.config/hypr/hyprdesk.lua`, one
`require("hypr.hyprdesk")` line added to `~/.config/hypr/hyprland.lua`;
full revert = delete file + line). Omarchy defaults load first, so each
override needs `hl.unbind` before `o.bind`. Number row is KEYCODE-based:
`code:10`..`code:19` = 1..0. (Pre-Quattro this was `hyprdesk.conf`
sourced from `hyprland.conf`, with `unbind`/`bindd`.)

| Keys                    | Now (Omarchy stock)        | Becomes            |
|-------------------------|----------------------------|--------------------|
| SUPER+1..0              | workspace 1..10            | switch to desk N   |
| SUPER+SHIFT+1..0        | movetoworkspace 1..10      | move win to desk N |
| SUPER+SHIFT+ALT+1..0    | movetoworkspacesilent 1..10| silent move desk N |
| SUPER+TAB               | workspace e+1              | next desk          |
| SUPER+SHIFT+TAB         | workspace e-1              | prev desk          |
| SUPER+CTRL+TAB          | workspace previous         | back-and-forth     |
| SUPER+scroll up/down    | workspace e-1/e+1          | prev/next desk     |

Untouched: SUPER+S scratchpad, SUPER+ALT+S move-to-scratchpad, ALT+TAB
window cycling, CTRL+ALT+TAB focusmonitor, SUPER+ALT+1..5 group switching.
SUPER+SHIFT+ALT+arrows: decide in sub-decisions above.

## Integration points (what to re-check after an Omarchy/Hyprland update)

The daemon's own logic has never broken on an update; the edges have.
When something stops working, check these four in order:

1. **Action grammar.** Does `hyprctl eval 'hl.dispatch(hl.dsp.focus({
   workspace = "1" }))'` still answer `ok`? The table above is the full
   set hyprdesk uses. Introspect the live API with
   `hyprctl eval` writing `pairs(hl)` to a file - `eval` does not echo
   return values, so dump to a file to read it.
2. **Config entry point.** Which file does the compositor actually load?
   `grep -i "using .* config" $XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/hyprland.log`.
   Our hook must be required from it.
3. **Binds.** `hyprctl binds -j`, matched on `description` (see above -
   the keycode fields are not populated). Expect 35 hyprdesk binds.
4. **Bar.** Whatever renders the desk strip. On Omarchy,
   `omarchy plugin list` should show `hyprdesk.desks` enabled, and
   `journalctl --user` carries its QML errors. On waybar, check that
   `hyprdesk waybar` still streams.

Validate config changes with `hyprctl reload` + `hyprctl configerrors`.
A human eyeballs the visual result; no headless screenshots.
