# Troubleshooting

Start with these two. Between them they identify most problems:

```bash
hyprdesk status          # is the daemon alive, and which desk does it think you are on?
hyprctl configerrors     # did the keybind file load cleanly?
```

---

## Nothing happens when I press SUPER+1

**Is the daemon running?**

```bash
hyprdesk status
```

If it says `cannot reach hyprdesk daemon`, it is not running. Start it by
hand to see why it is failing:

```bash
hyprdesk daemon
```

A healthy start prints `hyprdesk: started on desk N` and then sits there.
If it exits immediately, the message it prints says what went wrong.

**Are the keybinds loaded?**

```bash
hyprctl binds -j | grep -c 'Switch to desk'
```

Expect 10. If you get 0, the keybind file is not being loaded. Check that
`require("hypr.hyprdesk")` is in your `~/.config/hypr/hyprland.lua` and
that `~/.config/hypr/hyprdesk.lua` exists.

> Do not try to identify these binds by key. `hyprctl binds` reports
> keycode-based binds with `key: ""` and `keycode: 0`, and shows the
> dispatcher as an opaque `__lua` handle. Stock Hyprland binds look
> identical. Match on `description` instead.

## The daemon starts but exits right away

**`XDG_RUNTIME_DIR is not set` or `HYPRLAND_INSTANCE_SIGNATURE is not
set`** means it was launched outside a Hyprland session. Run it from a
terminal inside your session.

**`hyprland rejected a command: keyword can't work with non-legacy
parsers`** means you are on an old hyprdesk build against a newer
Hyprland. Rebuild: `cargo install --path . --root ~/.local --force`.

**`another hyprdesk daemon is already running`** is exactly what it says.
Find and stop the old one:

```bash
pgrep -x hyprdesk
kill <pid>
```

## The bar does not show desks

**On Omarchy**, check the widget is installed and enabled:

```bash
omarchy plugin list | grep -E 'hyprdesk.desks|omarchy.workspaces'
```

You want `hyprdesk.desks` enabled and `omarchy.workspaces` disabled. If
both are on you will see two strips; if both are off you will see none.

QML errors go to the journal:

```bash
journalctl --user -b | grep -i quickshell | tail -30
```

**Clicking a desk does nothing** usually means the widget cannot find the
binary. It runs `hyprdesk` through a login shell, so the install location
must be on your `PATH`:

```bash
bash -lc 'command -v hyprdesk'
```

If that prints nothing, add `~/.local/bin` (or wherever you installed it)
to your shell profile.

**On waybar**, confirm the stream works on its own:

```bash
hyprdesk waybar
```

It should print a JSON line immediately and another one every time
occupancy changes.

## Windows ended up on the wrong monitor

hyprdesk pins each workspace to a monitor with a workspace rule, and
re-asserts those rules whenever the config reloads or a monitor is plugged
in. If things look scrambled, force a re-weld by reloading:

```bash
hyprctl reload
```

If that fixes it, something wiped the rules without hyprdesk noticing.
Please open an issue with what you were doing.

**Do not use `SUPER+SHIFT+ALT+arrows`** to move a workspace between
monitors. hyprdesk unbinds it on purpose: moving a workspace off its
owning monitor breaks the desk weld, and the daemon will fight you on the
next switch.

## Windows did not come back after undocking and redocking

hyprdesk remembers where a window came from when its monitor disappears
and sends it back when that monitor slot returns. A window is deliberately
**not** sent back if:

- you moved it somewhere else while undocked, since that placement is
  treated as intentional, or
- it was closed and reopened, since it is a different window as far as the
  compositor is concerned.

Also note that memory is keyed to the monitor **slot**, not its name, so a
display re-enumerating under a different connector name still works.

## Windows are open but no desk shows them

You can hear an app or the bar shows a desk as occupied, but switching to
that desk shows nothing. List the workspaces:

```bash
hyprctl workspaces | grep '^workspace'
```

hyprdesk only uses workspaces `1-10` on the first monitor, `11-20` on the
second, and so on. A window on a workspace beyond your monitor count (for
example `31` with two monitors) is out of reach of every desk switch.

On Omarchy 4.5 and later, the usual cause is Omarchy's own per-monitor
workspaces (`/usr/share/omarchy/default/hypr/monitor-workspaces.lua`). It
gives each physical monitor a block of ten, remembered in
`~/.local/state/omarchy/monitor-workspaces`, and renumbers workspaces into
those blocks on every config reload and monitor change. A monitor it has
filed as its fourth gets `31-40` even when it is your second screen.
`~/.local/state/omarchy/monitor-workspaces-merged` lists each workspace it
renumbered and the number it had before.

`hyprdesk.lua` switches that renumbering off. Check that the switch is
in effect:

```bash
hyprctl eval "local f = io.open('/tmp/hyprdesk-reconcile', 'w'); local i = debug.getinfo(o.workspaces.reconcile, 'S'); f:write(i.short_src); f:close()"
cat /tmp/hyprdesk-reconcile   # expect .../hypr/hyprdesk.lua
```

To recover the windows, move each one to the same desk on the monitor it
is on, for example from `31` to `11`:

```bash
hyprctl eval 'hl.dispatch(hl.dsp.window.move({ window = "address:0x...", workspace = "11", follow = false }))'
```

Restarting the daemon also recovers them, but it moves them to the first
monitor, since their own monitor slot does not exist.

## It broke after a Hyprland or Omarchy update

The daemon's own logic has never broken on an update. The edges have.
`docs/DESIGN.md` ends with the five integration points to check in order:
the action grammar, the config entry point, the binds, the bar, and
Omarchy's own workspace placement. Work through that list.

If the action grammar changed, this is the fastest check:

```bash
hyprctl eval 'hl.dispatch(hl.dsp.focus({ workspace = "1" }))'
```

It must answer `ok`. The full set of actions hyprdesk uses is tabulated in
`docs/DESIGN.md`, and each one is pinned by a unit test in `src/hypr.rs`.

## Still stuck

Open an issue. The template asks for your Hyprland version, monitor
layout, and `hyprdesk status` output, which is almost always enough to
tell what is going on.
