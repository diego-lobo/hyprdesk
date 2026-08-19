-- hyprdesk - shared-monitor virtual desktops
-- https://github.com/diego-lobo/hyprdesk
--
-- Replaces Omarchy's stock per-monitor workspace bindings with desk
-- bindings: a desk spans ALL monitors and switches them together.
-- Full revert: delete this file and its require line in hyprland.lua.
--
-- Omarchy's defaults load before this module, so every override unbinds
-- first. The number row is keycode-based: code:10..code:19 = keys 1..0.
-- Untouched on purpose: SUPER+S scratchpad, ALT+TAB window cycling,
-- CTRL+ALT+TAB monitor focus, SUPER+ALT+scroll group cycling.

-- Resolve the binary at config-load time. Hyprland execs inherit the
-- session environment, whose PATH may be narrower than an interactive
-- shell's, so the standard no-sudo install locations are probed directly
-- and a bare name (PATH lookup) is the last resort.
local function resolve_hyprdesk()
  local home = os.getenv("HOME") or ""
  local candidates = {
    home .. "/.local/bin/hyprdesk",
    home .. "/.cargo/bin/hyprdesk",
    "/usr/local/bin/hyprdesk",
    "/usr/bin/hyprdesk",
  }

  for _, path in ipairs(candidates) do
    local file = io.open(path, "r")
    if file then
      file:close()
      return path
    end
  end

  return "hyprdesk"
end

local hyprdesk = resolve_hyprdesk()

local function run(subcommand)
  return hyprdesk .. " " .. subcommand
end

-- Start the daemon with the session. Unwrapped (not o.launch) so it
-- inherits the compositor's HYPRLAND_INSTANCE_SIGNATURE directly.
o.exec_on_start(run("daemon"))

-- Number row: switch desk, move window to desk, move it silently.
for desk = 1, 10 do
  local key = "code:" .. tostring(desk + 9)

  hl.unbind("SUPER + " .. key)
  hl.unbind("SUPER + SHIFT + " .. key)
  hl.unbind("SUPER + SHIFT + ALT + " .. key)

  o.bind("SUPER + " .. key, "Switch to desk " .. desk, run("vdesk " .. desk))
  o.bind("SUPER + SHIFT + " .. key, "Move window to desk " .. desk, run("movetodesk " .. desk))
  o.bind(
    "SUPER + SHIFT + ALT + " .. key,
    "Send window to desk " .. desk,
    run("movetodesksilent " .. desk)
  )
end

-- Cycle desks, and jump back to the previous one.
hl.unbind("SUPER + TAB")
hl.unbind("SUPER + SHIFT + TAB")
hl.unbind("SUPER + CTRL + TAB")

o.bind("SUPER + TAB", "Next desk", run("nextdesk --cycle"))
o.bind("SUPER + SHIFT + TAB", "Previous desk", run("prevdesk --cycle"))
o.bind("SUPER + CTRL + TAB", "Back-and-forth desk", run("lastdesk"))

-- Scroll to cycle desks, in the stock scroll orientation.
hl.unbind("SUPER + mouse_down")
hl.unbind("SUPER + mouse_up")

o.bind("SUPER + mouse_down", "Next desk", run("nextdesk --cycle"))
o.bind("SUPER + mouse_up", "Previous desk", run("prevdesk --cycle"))

-- SUPER+SHIFT+ALT+arrows: removed, no replacement. Moving a workspace
-- between monitors breaks the desk weld (a workspace would leave its
-- owning monitor); the daemon would fight it on the next switch.
for _, direction in ipairs({ "LEFT", "RIGHT", "UP", "DOWN" }) do
  hl.unbind("SUPER + SHIFT + ALT + " .. direction)
end
