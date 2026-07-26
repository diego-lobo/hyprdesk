# DESIGN - hyprdesk

Status: DECIDED 2026-07-26. Architecture B (IPC-based emulation) in Rust:
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
  headers, no sudo (see HANDOFF). A fork would build the same way, loaded
  via a `plugin = /home/diego/...so` config line.
- Cost: couples to Hyprland's INTERNAL headers, which drift every release
  (the v0.55.4-pin commit already needs a different include layout than the
  v0.55.3-pin commit). Every `pacman -Syu` that bumps Hyprland risks a
  rebuild-or-patch session. This is the exact hassle class we are escaping,
  now with us as the sole maintainer.
- Upside: true compositor-internal integration - perfectly atomic switches,
  first-class per-desk state.

### B. IPC-based emulation (RECOMMENDED)

Drive Hyprland exclusively through its STABLE public surface: `hyprctl`
dispatchers + `--batch`, workspace rules, and the socket2 event stream.
No compositor internals, so Hyprland updates cannot break the build - the
IPC surface is the compatibility contract.

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
  so nothing becomes unreachable. Redock does not un-merge (accepted).
- **Special/scratchpad workspace:** untouched; negative workspace ids are
  ignored by the daemon entirely.
- **SUPER+SHIFT+ALT+arrows** (`movecurrentworkspacetomonitor`): unbind at
  keybind-wiring time; it breaks the weld and has no desk-world meaning.
- **nextdesk/prevdesk:** bounded 1..=10; plain forms clamp, cycle forms
  wrap (upstream's unbounded desk creation does not fit a 10-key row).

## Keybind plan (agreed with Diego during investigation)

All overrides go in ONE new sourced file (e.g.
`~/.config/hypr/hyprdesk.conf`, one `source =` line added to
`~/.config/hypr/hyprland.conf`; full revert = delete file + line). Omarchy
defaults load first, so each override needs `unbind` before `bindd`.
Number row is KEYCODE-based: `code:10`..`code:19` = 1..0.

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

## Next steps (for the implementation session)

1. Read `reference/` source for exact upstream semantics (especially
   moveToDesk focus handling and hotplug re-weld logic).
2. Settle the A-vs-B pick and the B sub-decisions on the merits.
3. Decide, record the decision here (flip Status off DRAFT), scaffold the
   implementation (cargo init or scripts/), build, then wire keybinds last -
   config changes only after the tool works from the command line.
4. Validate every config step with `hyprctl reload` + `hyprctl configerrors`;
   Diego eyeballs the visual result (no headless screenshots).
