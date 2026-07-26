# HANDOFF - why hyprdesk exists (2026-07-26)

Diego wants macOS/Windows/KDE-style shared-monitor desktops on Omarchy:
every desk spans ALL monitors; eDP-1 and DP-2 switch together. The upstream
plugin `levnikmyskin/hyprland-virtual-desktops` does exactly this, but every
packaged install route failed on this system. Decision: build our own
solution emulating the plugin, for full custom control.

## Failure chain (all root-caused, none speculative)

1. **AUR `hyprland-plugin-virtual-desktops` 2.2.8-1**: hard-depends on
   `hyprland=0.52.2`; system has 0.55.4. Dead until the maintainer bumps.
2. **hyprpm from a Claude shell**: "Failed to write plugin state" - hyprpm
   stores state in root-owned `/var/cache/hyprpm/<user>/` BY DESIGN and
   elevates every state/header write via sudo (verified in hyprpm source at
   tag v0.55.4: `DataState.cpp` `writeState()` -> `NSys::root::install()`,
   escalation via sudo/doas/run0, no pkexec). No TTY = no password = fail.
   Even Diego's real-terminal run then hit:
3. **`hyprpm add` build failure**: the plugin's `hyprpm.toml` pin table maps
   Hyprland COMMIT HASHES to plugin commits. Its "v0.55.4" pin references
   Hyprland MASTER commit `14fa1fd0...` (where `Monitor.hpp` moved to
   `src/output/`), but Arch builds the RELEASE TAG commit `a0136d8c...`
   (Monitor still at `src/helpers/`). Same version string, different commit,
   different header layout. Pin lookup misses -> hyprpm silently builds
   plugin HEAD (0.56-targeting) -> compile error.
4. **Confirmed directly**: plugin commit `9575f360` (the v0.55.4 pin) fails
   with `fatal error: hyprland/src/output/Monitor.hpp: No such file`.

## What WORKS (proven, not theorized)

Plugin commit `70a1ae6c057c2906b36bad2185837fa8cc8a2a6c` (the v0.55.3 pin,
"fix moveToDesk ... on Hyprland 0.55+") **compiles cleanly against Arch's
system-shipped Hyprland 0.55.4 headers** with plain
`cmake -DCMAKE_BUILD_TYPE=Release && cmake --build` - no hyprpm, no sudo,
no PKG_CONFIG_PATH override. One harmless `[[deprecated]]` warning
(addConfigKeyword). Output `virtual-desktops.so` (2,430,224 bytes) is kept in
`reference/hyprland-virtual-desktops/` as a fallback artifact. Deps:
hyprland 0.55.4, libdrm, pixman-1, lua.

This proves: (a) system headers suffice for plugin builds; (b) the only real
long-term cost of the plugin route is chasing Hyprland's internal API drift
each release - which is exactly what the pin table exists to absorb and
exactly what broke.

## Open cleanup item

`/var/cache/hyprpm/<user>/` still contains the failed hyprpm install
(state.toml, headersRoot/, the plugin clone). Diego chowned it to
<user>:<user> mid-debug, then hyprpm's real-terminal run repopulated it.
None of it is needed. Cleanup = Diego runs **`hyprpm purge-cache`** in a real
terminal (needs sudo). Not yet confirmed done.

Nothing else was ever installed or modified: no AUR package landed, no
Hyprland/Omarchy config file was touched. System config is stock.

## Prior related experiment (context)

An earlier session tried `focusworkspaceoncurrentmonitor`-based scripting for
a similar goal and it was fully reverted to stock. The desired end state was
always welded all-monitor desks, not smarter per-monitor focus.
