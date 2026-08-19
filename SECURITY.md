# Security Policy

## Supported versions

The latest release on `main` is the supported version.

## Scope

hyprdesk runs entirely as an unprivileged user process. It contains no
`unsafe` code (`unsafe_code = "forbid"` at the crate level), requires no
`sudo` at any point, and installs nothing outside your home directory.

Its trust boundaries are:

- **`$XDG_RUNTIME_DIR/hyprdesk/<instance>.sock`** - the daemon's control
  socket, in your own user runtime directory. Anything that can write to
  it can already act as you.
- **Hyprland's own sockets** - hyprdesk sends `eval` chunks to the
  compositor. Compositor-supplied values (monitor names, window addresses)
  are escaped through `lua_quote` before interpolation, so a hostile
  window title or monitor name cannot inject Lua.

## Reporting a vulnerability

Please report privately through GitHub's
[security advisory form](https://github.com/diego-lobo/hyprdesk/security/advisories/new)
rather than a public issue.

Expect an acknowledgement within a week. This is a personal project
maintained in spare time, so please be patient with fix timelines.
