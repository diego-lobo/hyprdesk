# Upstream semantics - hyprland-virtual-desktops @ 70a1ae6c

Extracted 2026-07-26 from `reference/hyprland-virtual-desktops/` source (the
reference clone is gitignored; this doc is the durable record). File/line
references are to that commit.

## Desk model

- A vdesk is `{id, name, layouts}`. A layout maps monitor -> workspace id.
- Workspace mapping (`VirtualDesk::generateCurrentMonitorLayout`,
  VirtualDesk.cpp:173): desk `d` with `n` enabled monitors owns consecutive
  workspaces `(d-1)*n+1 .. d*n`, assigned to monitors in compositor order
  (`g_pCompositor->m_monitors` filtered by enabled, utils.cpp
  `currentlyEnabledMonitors`). Example, 2 monitors: desk 1 = ws 1,2; desk 2 =
  ws 3,4. NOTE: the same desk owns DIFFERENT workspace ids at different
  monitor counts (desk 2 undocked = ws 2), which is why layout memory exists.
- Layout memory (`rememberlayout`, default `size`): each vdesk keeps a list
  of layouts and re-activates the one matching the current monitor set, by
  count (`size`) or by monitor descriptions (`monitors`); `none` regenerates.
- Manual workspace switches are RECORDED into the active desk's layout
  (main.cpp `onWorkspaceChange` -> `changeWorkspaceOnMonitor`), so a desk's
  contents follow what the user does, suppressed while
  `monitorLayoutChanging` is set (hotplug in progress).

## Dispatchers (all registered in main.cpp PLUGIN_INIT)

- `vdesk N|name` (`changeActiveDesk`): if target == active, run
  `cycleWorkspaces()` instead - with exactly 2 monitors it swaps the two
  active workspaces (`swapActiveWorkspaces`) and records the swap in the
  layout; gated on `cycleworkspaces` (default 1); >2 monitors unimplemented.
  Otherwise: `lastDesk = active`, `active = N` (creating the vdesk if new),
  then apply.
- Apply (`applyCurrentVDesk`, VirtualDeskManager.cpp:56): for each
  (monitor, wsId) in the active layout: create workspace if missing; if
  workspace sits on the wrong monitor, `moveWorkspaceToMonitor`; then
  `monitor->changeWorkspace(ws)` for every monitor EXCEPT the focused one,
  which is done LAST so focus (and cursor) stay on it. Emits custom IPC
  event `vdesk` with the new desk id (for waybar etc.).
- `lastdesk`: back-and-forth to `lastDesk` (-1 = never switched = no-op).
- `prevdesk`: `active-1`, clamped to 1. `backcyclevdesks`: same but wraps to
  the max EXISTING desk id.
- `nextdesk`: `active+1`, unbounded (implicitly creates new desks).
  `cyclevdesks`: `active+1` if that desk exists, else 1.
- `movetodesk N[,winregex]` (`moveToDesk`, VirtualDeskManager.cpp:100):
  window = regex match or focused window; target workspace = desk N's
  workspace ON THE WINDOW'S MONITOR (fallback: first workspace of the
  layout); create it if missing; `moveWindowToWorkspaceSafe`; then the
  non-silent variant FOLLOWS with a full `changeActiveDesk(N)`.
  `movetodesksilent`: move only, no switch. `movetolastdesk[silent]`,
  `movetoprevdesk[silent]`, `movetonextdesk[silent]`: same with target from
  lastDesk/prevDeskId/nextDeskId; prev/next take a leading `1,` arg for
  cycle mode.
- `vdeskreset [N|name]`: regenerate layout(s) from the arithmetic default,
  re-apply, re-match sticky rules.
- hyprctl commands: `printdesk [N|name]`, `printstate`, `printlayout`
  (normal + JSON formats) for introspection.

## Events (main.cpp hooks)

- `monitor.preAdded`/`preRemoved`: set `monitorLayoutChanging = true`
  (suppress workspace recording during the transition). `HEADLESS-1` is
  ignored everywhere.
- `monitor.added` / `removed` (with >=1 enabled monitor left): clear the
  flag, invalidate all layouts, re-match/repair layouts (orphaned layout
  entries reassigned; replacement monitor chosen as the enabled monitor
  whose active workspace has the FEWEST windows,
  `VirtualDesk::firstAvailableMonitor`), re-apply current desk, re-match
  sticky rules.
- `window.open`: sticky-rule match can force the window to its desk and
  switch to it.
- `config.reloaded`: parse `names` config, one-shot init notification.

## Config surface (globals.hpp, all under plugin:virtual-desktops:)

- `names` (string, "unset"): `id:name,...` map.
- `cycleworkspaces` (int, 1): same-desk re-press swaps workspaces (2 mons).
- `rememberlayout` (string -> "size"): none | size | monitors.
- `notifyinit` (int, 1), `verbose_logging` (int, 0).
- `stickyrule` keyword (sticky_apps.cpp): `class-or-title-match, vdesk`.

## hyprdesk deviations (decided, see DESIGN.md)

- Fixed arithmetic mapping `ws = d + 10*monitor_index` instead of
  consecutive-block mapping + layout memory.
- Same-desk re-press = no-op in v1 (upstream's swap fights workspace->monitor
  pinning rules); revisit later.
- Sticky apps deferred (nice-to-have).
- `nextdesk` bounded by MAX_DESKS=10 instead of unbounded desk creation.
