#!/usr/bin/env bash
# Re-fetch the upstream plugin reference at the pinned commit.
# The pin (70a1ae6c, the upstream v0.55.3 hyprpm pin) is the one commit
# PROVEN to compile against this system's Hyprland 0.55.4 headers.
set -euo pipefail

PIN=70a1ae6c057c2906b36bad2185837fa8cc8a2a6c
URL=https://github.com/levnikmyskin/hyprland-virtual-desktops
DEST="$(cd "$(dirname "$0")/.." && pwd)/reference/hyprland-virtual-desktops"

if [[ -e "$DEST" ]]; then
    echo "already exists: $DEST" >&2
    exit 1
fi

git clone "$URL" "$DEST"
git -C "$DEST" checkout "$PIN"
echo "reference ready at $DEST (pinned $PIN)"
