# Why hyprdesk exists

> Historical record, written 2026-07-26. Every version number below is
> from before Omarchy 4 (Hyprland 0.55.4, Omarchy 3). The reasoning still
> stands, since it is why the project exists. For the current design see
> [`DESIGN.md`](DESIGN.md).

The goal was macOS/Windows/KDE-style shared-monitor desktops on Hyprland:
every desk spans all monitors, and the laptop panel and the external
display switch together.

[`levnikmyskin/hyprland-virtual-desktops`](https://github.com/levnikmyskin/hyprland-virtual-desktops)
does exactly this, and does it well. Every packaged route to installing it
failed. Rather than fight the packaging, hyprdesk emulates the behavior
over Hyprland's stable IPC, where none of these failure modes exist.

## Failure chain (all root-caused, none speculative)

1. **AUR `hyprland-plugin-virtual-desktops` 2.2.8-1** hard-depends on
   `hyprland=0.52.2`; the system had 0.55.4. Dead until the maintainer
   bumps it. This is structural: a compositor plugin has to be pinned to a
   compositor version, so every Hyprland release breaks the package until
   somebody rebuilds it.
2. **`hyprpm` needs root.** It stores state in root-owned
   `/var/cache/hyprpm/<user>/` **by design** and elevates every state and
   header write through sudo (verified in hyprpm source at tag v0.55.4:
   `DataState.cpp` `writeState()` calling `NSys::root::install()`, with
   escalation via sudo/doas/run0 and no pkexec path). Any environment
   without an interactive terminal cannot install a plugin at all.
3. **`hyprpm add` then failed on its own pin table.** The plugin's
   `hyprpm.toml` maps Hyprland **commit hashes** to plugin commits. Its
   "v0.55.4" pin references Hyprland *master* commit `14fa1fd0...`, where
   `Monitor.hpp` had moved to `src/output/`, but Arch builds the *release
   tag* commit `a0136d8c...`, where Monitor is still at `src/helpers/`.
   Same version string, different commit, different header layout. The pin
   lookup missed, hyprpm silently fell back to building plugin HEAD (which
   targets 0.56), and that failed to compile.
4. **Confirmed directly:** plugin commit `9575f360` (the v0.55.4 pin)
   fails with `fatal error: hyprland/src/output/Monitor.hpp: No such file`.

## What did work

Plugin commit `70a1ae6c057c2906b36bad2185837fa8cc8a2a6c` (the v0.55.3 pin,
"fix moveToDesk ... on Hyprland 0.55+") **compiles cleanly against Arch's
system-shipped Hyprland 0.55.4 headers** with plain
`cmake -DCMAKE_BUILD_TYPE=Release && cmake --build`. No hyprpm, no sudo, no
`PKG_CONFIG_PATH` override. One harmless `[[deprecated]]` warning
(`addConfigKeyword`). Dependencies: hyprland 0.55.4, libdrm, pixman-1, lua.

This proves two things:

- **The system headers suffice**, so the plugin route was never blocked by
  the toolchain. Only by `hyprpm`'s state machinery and its pin table.
- **The real long-term cost of the plugin route is chasing Hyprland's
  internal API drift every release**, which is exactly what the pin table
  exists to absorb and exactly what broke.

## The conclusion

A plugin buys perfectly atomic switching at the price of a rebuild-or-patch
session on every compositor update, plus a root-owned installer. An IPC
client gives up compositor-internal atomicity and gets a public contract
that does not move.

hyprdesk took the IPC route, and the bet paid off: the Hyprland 0.56
upgrade, which changed the compositor's entire config and action grammar,
cost a single file. The daemon's logic did not change at all. See the port
section of [`DESIGN.md`](DESIGN.md).

## Prior related experiment

An earlier attempt used `focusworkspaceoncurrentmonitor`-based scripting
for a similar goal and was fully reverted. The desired end state was
always welded all-monitor desks, not smarter per-monitor focus.
