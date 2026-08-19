#!/usr/bin/env bash
# Install hyprdesk: build the binary, wire the Hyprland keybinds, and (on
# Omarchy) enable the desk bar widget. No sudo, no system paths.
#
# Everything it touches is reversible with scripts/uninstall.sh.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${HYPRDESK_PREFIX:-$HOME/.local}"
HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
OMARCHY_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy"
ENTRY="$HYPR_DIR/hyprland.lua"
REQUIRE_LINE='require("hypr.hyprdesk")'

say() { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33m warn\033[0m %s\n' "$1" >&2; }
die() { printf '\033[1;31merror\033[0m %s\n' "$1" >&2; exit 1; }

# 1. Build and install the binary.
command -v cargo >/dev/null || die "cargo not found. Install Rust from https://rustup.rs"

say "Building hyprdesk (release)"
cargo install --path "$REPO" --root "$PREFIX" --force --quiet

BIN="$PREFIX/bin/hyprdesk"
[[ -x $BIN ]] || die "expected the binary at $BIN but it is not there"
say "Installed $BIN"

case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) warn "$PREFIX/bin is not on your PATH. Add it to your shell profile, or the bar widget will not be able to switch desks." ;;
esac

# 2. Link the keybind module into the Hyprland config directory.
[[ -d $HYPR_DIR ]] || die "$HYPR_DIR does not exist. Is Hyprland installed for this user?"

LINK="$HYPR_DIR/hyprdesk.lua"
if [[ -e $LINK && ! -L $LINK ]]; then
  die "$LINK exists and is a regular file. Move it aside, then re-run."
fi
ln -sfn "$REPO/config/hypr/hyprdesk.lua" "$LINK"
say "Linked $LINK"

# 3. Require it from the config entry point, after the defaults.
if [[ ! -f $ENTRY ]]; then
  warn "No $ENTRY found. Add this line to your Hyprland Lua config yourself:"
  warn "    $REQUIRE_LINE"
elif grep -qF "$REQUIRE_LINE" "$ENTRY"; then
  say "Already required from hyprland.lua"
else
  cp "$ENTRY" "$ENTRY.hyprdesk-backup.$(date +%s)"
  printf '\n-- hyprdesk: shared-monitor virtual desktops\n%s\n' "$REQUIRE_LINE" >>"$ENTRY"
  say "Appended $REQUIRE_LINE to hyprland.lua (backup kept alongside it)"
fi

# 4. Omarchy only: install and enable the desk bar widget.
if [[ -d $OMARCHY_DIR ]] && command -v omarchy >/dev/null; then
  PLUGIN_LINK="$OMARCHY_DIR/plugins/hyprdesk.desks"
  mkdir -p "$OMARCHY_DIR/plugins"
  if [[ -e $PLUGIN_LINK && ! -L $PLUGIN_LINK ]]; then
    warn "$PLUGIN_LINK exists and is not a symlink; leaving it alone."
  else
    ln -sfn "$REPO/config/omarchy/plugins/hyprdesk.desks" "$PLUGIN_LINK"
    say "Linked $PLUGIN_LINK"
    # Take the slot the stock workspaces widget was in, so the bar layout
    # looks the same as before.
    omarchy plugin disable omarchy.workspaces >/dev/null 2>&1 || true
    if omarchy plugin enable hyprdesk.desks --section left --index 0 >/dev/null 2>&1; then
      say "Enabled the Desks bar widget (stock workspaces widget disabled)"
    else
      warn "Could not enable hyprdesk.desks automatically. Run: omarchy plugin enable hyprdesk.desks --section left --index 0"
    fi
  fi
else
  say "Omarchy not detected; skipping the bar widget (see README for waybar)"
fi

# 5. Apply.
say "Reloading Hyprland"
hyprctl reload >/dev/null
errors="$(hyprctl configerrors 2>/dev/null || true)"
if [[ -n $errors && $errors != no\ config\ errors* ]]; then
  warn "Hyprland reported config errors; run 'hyprctl configerrors' to see them."
fi

cat <<'DONE'

hyprdesk is installed. Try it:

  SUPER + 1..0            switch every monitor to that desk
  SUPER + SHIFT + 1..0    move the focused window there and follow
  SUPER + TAB             next desk

Log out and back in if the daemon did not start; check it with
'hyprdesk status'. To remove everything: scripts/uninstall.sh
DONE
