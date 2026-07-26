# DESIGN - hyprdesk

Status: DRAFT scaffold. The architecture pick below is a RECOMMENDATION from
the investigating session, not a decision. Red-team it with Codex (see
CLAUDE.md decision policy) before building.

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

Known caveat to evaluate honestly: a batched multi-dispatch switch is not
compositor-atomic; there may be a visible one-frame stagger between
monitors. Measure it in practice before declaring it acceptable (upstream
plugin A does not have this issue).

### Sub-decisions for B (open)

- Stateless per-keypress CLI vs. small resident daemon (hotplug + waybar
  push argue for a tiny daemon or a CLI + separate listener).
- Language: bash (fastest to iterate) vs. Rust (Diego's main toolchain,
  single static binary, raw UNIX-socket IPC is dependency-free; socket path
  `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket{,2}.sock`).
- Focus behavior after switch (which monitor keeps focus); interaction with
  the special/scratchpad workspace; what SUPER+SHIFT+ALT+arrows
  (`movecurrentworkspacetomonitor`) should mean once desks are welded -
  probably unbind or repurpose, since it breaks the weld.

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
2. Codex red-team the A-vs-B pick and the B sub-decisions (independent
   research, do not feed it this doc's conclusions first).
3. Decide, record the decision here (flip Status off DRAFT), scaffold the
   implementation (cargo init or scripts/), build, then wire keybinds last -
   config changes only after the tool works from the command line.
4. Validate every config step with `hyprctl reload` + `hyprctl configerrors`;
   Diego eyeballs the visual result (no headless screenshots).
