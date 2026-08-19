#!/usr/bin/env bash
# Remove everything scripts/install.sh put in place and restore the stock
# Hyprland/Omarchy behavior. Leaves your windows and workspaces alone.
set -euo pipefail

PREFIX="${HYPRDESK_PREFIX:-$HOME/.local}"
HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
OMARCHY_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy"
ENTRY="$HYPR_DIR/hyprland.lua"
REQUIRE_LINE='require("hypr.hyprdesk")'

say() { printf '\033[1;34m==>\033[0m %s\n' "$1"; }

# 1. Stop the daemon. Matched on the exact process name, never a pattern.
if pgrep -x hyprdesk >/dev/null 2>&1; then
  pkill -x hyprdesk || true
  say "Stopped the daemon"
fi

# 2. Unwire the keybind module.
if [[ -f $ENTRY ]] && grep -qF "$REQUIRE_LINE" "$ENTRY"; then
  cp "$ENTRY" "$ENTRY.hyprdesk-backup.$(date +%s)"
  grep -vF "$REQUIRE_LINE" "$ENTRY" >"$ENTRY.tmp" || true
  mv "$ENTRY.tmp" "$ENTRY"
  say "Removed the require line from hyprland.lua (backup kept alongside it)"
fi

if [[ -L "$HYPR_DIR/hyprdesk.lua" ]]; then
  rm "$HYPR_DIR/hyprdesk.lua"
  say "Unlinked hyprdesk.lua"
fi

# 3. Omarchy only: restore the stock workspaces widget.
if [[ -L "$OMARCHY_DIR/plugins/hyprdesk.desks" ]]; then
  if command -v omarchy >/dev/null; then
    omarchy plugin disable hyprdesk.desks >/dev/null 2>&1 || true
  fi
  rm "$OMARCHY_DIR/plugins/hyprdesk.desks"
  if command -v omarchy >/dev/null; then
    omarchy plugin enable omarchy.workspaces >/dev/null 2>&1 || true
  fi
  say "Removed the Desks widget and re-enabled the stock workspaces widget"
fi

# 4. Remove the binary.
if [[ -x "$PREFIX/bin/hyprdesk" ]]; then
  rm "$PREFIX/bin/hyprdesk"
  say "Removed $PREFIX/bin/hyprdesk"
fi

hyprctl reload >/dev/null 2>&1 || true
say "Done. Your stock per-monitor workspaces are back."
