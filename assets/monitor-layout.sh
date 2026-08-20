#!/usr/bin/env bash
# Emit a gpu-screen-recorder capture spec that puts every monitor in one frame.
#
# Two layouts:
#   side-by-side (default)  monitors left to right at native resolution, the
#                           way they sit on the desk
#   stacked                 monitors top to bottom, each drawn in proportion to
#                           its real physical size, largest first
#
# Prints two lines: the spec for `gpu-screen-recorder -w`, then the WxH content
# box. The box matters because gsr sizes its canvas from each source's native
# resolution, so a scaled layout leaves black padding to crop off afterwards.
#
# Usage:
#   spec=$(assets/monitor-layout.sh stacked | head -1)
#   box=$(assets/monitor-layout.sh stacked | tail -1)
#   gpu-screen-recorder -w "$spec" -f 30 -o /tmp/demo.mp4
set -euo pipefail

layout="${1:-side-by-side}"
case $layout in
side-by-side | stacked) ;;
*)
  printf 'usage: %s [side-by-side|stacked]\n' "${0##*/}" >&2
  exit 2
  ;;
esac

command -v hyprctl >/dev/null || { echo "hyprctl not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq not found" >&2; exit 1; }

# name, pixel size, and physical size in mm. Hyprland does not report physical
# dimensions, so they come from the EDID: detailed timing descriptor 1 starts
# at byte 54, and bytes 66/67/68 hold the image size with the high nibbles of
# both axes packed into byte 68. The "Maximum image size" in the EDID header is
# rounded to whole centimetres and is simply wrong on some panels, so it is not
# used here.
monitors=$(
  hyprctl monitors -j | jq -r '.[] | select(.disabled | not) | "\(.name) \(.width) \(.height) \(.x) \(.y)"' |
    while read -r name px py mx my; do
      edid=$(echo /sys/class/drm/card*-"$name"/edid)
      [[ -r $edid ]] || { echo "no EDID for $name, skipping" >&2; continue; }
      read -r b66 b67 b68 < <(od -An -tu1 -j66 -N3 "$edid")
      mm_w=$((b66 + (b68 / 16) * 256))
      mm_h=$((b67 + (b68 % 16) * 256))
      ((mm_w > 0 && mm_h > 0)) || { echo "EDID for $name has no image size, skipping" >&2; continue; }
      printf '%s %s %s %s %s %s %s\n' "$name" "$px" "$py" "$mm_w" "$mm_h" "$mx" "$my"
    done
)

[[ -n $monitors ]] || { echo "no usable monitors found" >&2; exit 1; }

awk -v layout="$layout" '
function even(v) { return int(v / 2) * 2 }

{ n++; name[n] = $1; pw[n] = $2; ph[n] = $3; mw[n] = $4; mh[n] = $5; lx[n] = $6; ly[n] = $7 }

END {
  for (i = 1; i <= n; i++) ord[i] = i

  if (layout == "side-by-side") {
    # Desk order, native resolution, top aligned. Left to right, then top to
    # bottom, so a vertically stacked desktop still orders deterministically.
    for (i = 1; i <= n; i++)
      for (j = i + 1; j <= n; j++)
        if (lx[ord[j]] < lx[ord[i]] ||
            (lx[ord[j]] == lx[ord[i]] && ly[ord[j]] < ly[ord[i]])) { t = ord[i]; ord[i] = ord[j]; ord[j] = t }

    x = 0; boxh = 0
    for (k = 1; k <= n; k++) {
      i = ord[k]
      printf "%s%s;x=%d;y=0;width=%d;height=%d;halign=start;valign=start", sep, name[i], x, pw[i], ph[i]
      sep = "|"; x += pw[i]
      if (ph[i] > boxh) boxh = ph[i]
    }
    printf "\n%dx%d\n", x, boxh
    exit
  }

  # Stacked: one shared pixels-per-mm for every panel, anchored to the least
  # dense one so nothing is ever upscaled past its native resolution.
  for (i = 1; i <= n; i++) {
    ppmm = pw[i] / mw[i]
    if (scale == 0 || ppmm < scale) scale = ppmm
  }

  # Physically largest on top.
  for (i = 1; i <= n; i++)
    for (j = i + 1; j <= n; j++)
      if (mw[ord[j]] > mw[ord[i]]) { t = ord[i]; ord[i] = ord[j]; ord[j] = t }

  boxw = 0
  for (i = 1; i <= n; i++) {
    rw[i] = even(mw[i] * scale); rh[i] = even(mh[i] * scale)
    if (rw[i] > pw[i]) { rw[i] = pw[i]; rh[i] = ph[i] }
    if (rw[i] > boxw) boxw = rw[i]
  }

  y = 0
  for (k = 1; k <= n; k++) {
    i = ord[k]
    printf "%s%s;x=%d;y=%d;width=%d;height=%d;halign=start;valign=start", \
      sep, name[i], even((boxw - rw[i]) / 2), y, rw[i], rh[i]
    sep = "|"; y += rh[i]
  }
  printf "\n%dx%d\n", boxw, y
}' <<<"$monitors"
