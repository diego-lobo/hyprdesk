#!/usr/bin/env bash
# Record every monitor into one frame, cropped, in a single step.
#
# gpu-screen-recorder sizes its canvas from the bounding box of each source's
# NATIVE resolution at its requested position, ignoring the size it was asked
# to draw at, so a scaled layout lands in the top-left corner of a much larger
# black frame. No recorder option declares the canvas up front: -s is ignored
# for multi-source captures, and percentage placement re-anchors the canvas to
# the largest source while squashing the others out of aspect. So the padding
# is cropped off here, right after the recording stops.
#
# Usage:
#   assets/record-demo.sh [stacked|side-by-side] [output.mp4]
#
# Perform the demo, then press Ctrl+C. The crop runs on its own afterwards.
set -euo pipefail

layout="${1:-stacked}"
out="${2:-/tmp/hyprdesk-demo.mp4}"

command -v gpu-screen-recorder >/dev/null || { echo "gpu-screen-recorder not found" >&2; exit 1; }
command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mapfile -t plan < <("$here/monitor-layout.sh" "$layout")
spec="${plan[0]:-}"
box="${plan[1]:-}"
[[ -n $spec && -n $box ]] || { echo "could not work out a capture layout" >&2; exit 1; }

raw=$(mktemp --suffix=.mp4 -t hyprdesk-raw.XXXXXX)
trap 'rm -f "$raw"' EXIT

printf 'layout:  %s\ncontent: %s\nrecording, press Ctrl+C to stop\n' "$layout" "$box"

gpu-screen-recorder -w "$spec" -f 30 -o "$raw" &
gsr=$!

# gsr needs SIGINT to finalise the container; anything harsher leaves an
# unplayable file. Ctrl+C reaches it anyway as part of the foreground process
# group, but signal it explicitly so the script also works when driven from
# another terminal or a hook. It is signalled by exact pid because the kernel
# truncates its process name to 15 characters ("gpu-screen-reco"), so
# `pkill -x gpu-screen-recorder` silently matches nothing.
trap 'kill -INT "$gsr" 2>/dev/null || true' INT
wait "$gsr" 2>/dev/null || true
trap - INT
# `wait` returns as soon as the trap fires, which can be before gsr has
# finished writing the container. Let it actually exit.
while kill -0 "$gsr" 2>/dev/null; do sleep 0.2; done

[[ -s $raw ]] || { echo "the recorder produced nothing" >&2; exit 1; }

printf 'cropping to %s\n' "$box"
ffmpeg -y -v error -i "$raw" \
  -vf "crop=${box%x*}:${box#*x}:0:0" \
  -c:v libx264 -crf 18 -pix_fmt yuv420p "$out"

printf 'wrote %s (%s)\n' "$out" "$box"
