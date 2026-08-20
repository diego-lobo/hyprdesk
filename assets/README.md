# Assets

`demo.gif`, `bar.png`, and `social-preview.png` are real captures of a live
two-monitor session. This file records how each one was made, so any of
them can be regenerated after a UI change without rediscovering the
mechanics.

No video editor is needed for any of this. Everything below is one
recorder process and one `ffmpeg` command.

## `demo.gif` - the one that sells the project

This is the single most valuable asset in the repo. hyprdesk is a
multi-monitor tool, so a terminal recording cannot show what it does: the
point is *two screens changing at the same time*. It needs a real screen
capture that includes every monitor in one frame.

**What to capture,** in about 10 to 15 seconds:

1. Start on desk 1 with something recognisable open on each monitor, for
   example a browser on the external and a terminal on the laptop.
2. Press `SUPER+2`. Both monitors change together. Let it sit for a beat
   so the viewer registers that both moved.
3. Press `SUPER+3`, then `SUPER+1` to come back.
4. Optionally finish by dragging a window with `SUPER+SHIFT+2`.

Keep the mouse still and in frame. The whole argument is that you never
had to move it, and a parked cursor is what proves that.

### Recording every monitor into one file

Wayland has no single framebuffer spanning all outputs, so most recorders
(`wf-recorder`, `wl-screenrec`, the portal path, and `omarchy capture
screenrecording --fullscreen`) capture exactly **one** output.

`gpu-screen-recorder` 6.0+, which Omarchy already ships and uses, solves
this a different way: it composites **several capture sources onto one
canvas** in a single process. Sources are joined with `|` and each one
takes `;key=value` options for its placement. That is the mechanism to
use, and it needs no stitching and no editor.

[`record-demo.sh`](record-demo.sh) is the whole job in one command. Perform
the demo, press `Ctrl+C`, and it writes a finished, tightly cropped file:

```bash
./assets/record-demo.sh stacked        # or: side-by-side
# ...perform the demo, then Ctrl+C
# wrote /tmp/hyprdesk-demo.mp4 (2560x2018)
```

Pass a second argument to choose the output path. It picks the layout up
from whatever monitors are attached, so it needs no configuration.

<details>
<summary><b>What it does, and why the crop is not optional</b></summary>

[`monitor-layout.sh`](monitor-layout.sh) builds the capture spec from the
live monitor layout, printing the spec on the first line and a `WxH`
content box on the second:

```bash
spec=$(./assets/monitor-layout.sh stacked | head -1)
box=$(./assets/monitor-layout.sh stacked | tail -1)

gpu-screen-recorder -w "$spec" -f 30 -o /tmp/hyprdesk-raw.mp4
```

Stop it with `Ctrl+C`. SIGINT is required, because anything harsher leaves
the container unfinalised. To stop it from another terminal, signal it by
**pid**, not by name: the kernel truncates its process name to 15
characters, so `pkill -x gpu-screen-recorder` matches nothing at all and
looks like the recorder ignoring you.

**The raw file has a large black margin, and cropping it is the second
half of the recording step, not a cosmetic pass:**

```bash
ffmpeg -y -i /tmp/hyprdesk-raw.mp4 \
  -vf "crop=${box%x*}:${box#*x}:0:0" -c:v libx264 -crf 18 \
  /tmp/hyprdesk-demo.mp4
```

gsr sizes its canvas from the bounding box of every source's **native**
resolution at its requested position, ignoring the sizes you asked it to
draw at, so a scaled layout lands in the top-left corner of a much larger
frame. Cropping to the content box removes every pixel of that exactly.

Nothing declares the canvas up front, which is why the crop exists.
`-s WxH` is ignored for multi-source captures. Percentage positions do
re-anchor the canvas, but to the largest single source, squashing the
others out of aspect. Piping gsr's stdout straight into an `ffmpeg` crop
does work, but it needs a fifo, spends CPU re-encoding the full padded
frame live, and truncates the container tail, so the script records to a
temporary file and crops afterwards instead.
</details>

**side-by-side** mirrors how the monitors sit on the desk, every pixel
1:1. A 2560x1600 laptop beside a 2560x1440 external gives a 5120x1600
frame, with a black strip under the shorter panel.

**stacked** draws each monitor in proportion to its real physical size,
physically largest on top, horizontally centred. This is the arrangement
that reads as "an external monitor with a laptop in front of it", and it
is much closer to a 16:9 shape than the very wide side-by-side frame. A
27 inch external above a 10 inch laptop gives a 2560x2018 box in which the
laptop is 930px wide, because that is genuinely how big it is next to the
external. Budget for that: once cropped, the external fills the full frame
width and the laptop leaves a black band either side of it worth about 18%
of the frame. That share is true scale itself, not an artifact, and the
only ways to shrink it are to give up true scale or to use side-by-side.

Physical dimensions come from each monitor's EDID, since Hyprland does not
report them. The script reads the detailed timing descriptor rather than
the "Maximum image size" field in the EDID header, which is rounded to
whole centimetres and is flatly wrong on some panels.

Two more things to know about the recorder itself:

- **Set `width` and `height` on every source** (the script always does).
  Left out, gsr scales each source to the canvas height, so a shorter
  monitor comes out stretched and the canvas grows to a strange size.
- **Do not use `-w region`.** Region capture resolves to the single
  monitor containing the region and does not composite across outputs, so
  a box larger than that monitor gives you its contents repeated rather
  than your other screen.

A side-by-side two-monitor canvas exceeds H.264's 4096px width limit, so
the recorder switches to HEVC on its own. Stacked layouts usually stay
under it and encode as H.264. Either is fine; `ffmpeg` reads both.

### Converting to a GIF

From the cropped file, not the raw one:

```bash
ffmpeg -y -i /tmp/hyprdesk-demo.mp4 \
  -vf "fps=12,scale=900:-1:flags=lanczos,palettegen=stats_mode=diff" \
  /tmp/palette.png

ffmpeg -y -i /tmp/hyprdesk-demo.mp4 -i /tmp/palette.png \
  -lavfi "fps=12,scale=900:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3" \
  -loop 0 assets/demo.gif
```

Two passes rather than one `split`: `stats_mode=diff` builds the palette
from what actually changes between frames, which is the right trade for a
desktop capture where most pixels are static.

Size depends far more on how much of the screen changes than on how long
the clip is. A mostly static desktop lands near 90 KB per second of video;
a busy one with a browser, a video, or an animated wallpaper runs several
times that. The shipped `demo.gif` is 20 seconds of a busy two-monitor
desktop and comes to 6.6 MB. To bring one down, drop `scale` to 800 or
`fps` to 10 before touching anything else, or shorten the clip with `-t`.

To trim the ends, `ffmpeg -ss 2 -t 14 -i in.mp4 -c copy out.mp4` is
enough. If you want a GUI for that, LosslessCut is the lightest thing
worth installing, but it is genuinely optional.

### Fallbacks

On `gpu-screen-recorder` older than 6.0, multi-source compositing is not
available. Record each monitor separately and stack them afterwards; still
no editor:

```bash
gpu-screen-recorder -w eDP-1     -f 30 -o /tmp/left.mp4 &  left=$!
gpu-screen-recorder -w HDMI-A-1  -f 30 -o /tmp/right.mp4 & right=$!
# ...perform the demo, then:
kill -INT "$left" "$right"

ffmpeg -i /tmp/left.mp4 -i /tmp/right.mp4 \
  -filter_complex "[0:v]pad=iw:1600:0:0[l];[l][1:v]hstack=inputs=2:shortest=1" /tmp/stacked.mp4
```

`hstack` needs matching heights, hence the `pad`. The two recorders start
milliseconds apart, which is invisible at GIF frame rates but is why the
single-process capture above is still preferred.

OBS also does this, with one "Screen Capture (PipeWire)" source per
monitor arranged on a single canvas. It works and needs no stitching, but
it is a much heavier route to the same file.

## `bar.png` - the desk strip

A tight crop of the Omarchy bar showing the desk indicators. The widget
draws three states, and a good frame shows all of them at once: the active
desk becomes a filled dot instead of a number, occupied desks are bright,
and empty ones are dimmed. Pick a moment when the active desk is not the
first one, so it is obvious the dot marks a position in the strip.

The demo recording already contains the bar at full resolution, so it is
the easiest source. The strip sits in the top left, ending just before the
tiling-layout icon, which belongs to Omarchy rather than to hyprdesk and
is cropped out:

```bash
ffmpeg -v error -ss 7 -i /tmp/hyprdesk-demo.mp4 -frames:v 1 /tmp/frame.png
magick /tmp/frame.png -crop 136x32+6+0 +repage \
  -filter Lanczos -resize 400% assets/bar.png
```

Measure the crop rather than reusing those numbers. Bar height and widget
order both shift with monitor scale and with the configured bar layout;
`136x32` is what a 2560px wide capture at scale 1.25 happens to give. The
filter is worth specifying, because a strip this small comes out visibly
jagged under point sampling and mushy under Mitchell.

A live screenshot works too, if you would rather not go via the recording:

```bash
omarchy capture screenshot region save
```

## `social-preview.png` - the link card

GitHub shows this image whenever the repo is linked on Reddit, Discord, or
anywhere else that expands links, which is exactly where a Hyprland tool
gets found. Without one, the card falls back to GitHub's generic panel of
repo name, avatar, and star count. It is a still: no platform animates a
link preview, so `demo.gif` cannot do this job.

The file lives here so it can be regenerated, but committing it does
nothing by itself. GitHub only uses it once it is uploaded under Settings,
then General, then Social preview, and it has to be re-uploaded after any
change.

It is **1280x640**, the size GitHub documents. The two monitors are
composited separately rather than dropped in as one stacked frame, because
the stacked frame carries black bands either side of the laptop that read
as dead space once scaled down to a card:

```bash
ffmpeg -v error -ss 8 -i /tmp/hyprdesk-demo.mp4 -frames:v 1 /tmp/frame.png

# Geometry from monitor-layout.sh stacked: external 2560x1440+0+0,
# laptop 930x578+814+1440.
magick /tmp/frame.png -crop 2560x1440+0+0 +repage -resize 640x \
  -bordercolor '#31313f' -border 2 /tmp/ext.png
magick /tmp/frame.png -crop 930x578+814+1440 +repage -resize 232x \
  -bordercolor '#31313f' -border 2 /tmp/lap.png

magick -size 1280x640 gradient:'#15151e-#0a0a10' \
  \( /tmp/ext.png \) -gravity northwest -geometry +578+72 -composite \
  \( /tmp/lap.png \) -gravity northwest -geometry +782+448 -composite \
  -font /usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf \
  -fill '#f4f4f9' -pointsize 76 -annotate +64+246 'hyprdesk' \
  -font /usr/share/fonts/liberation/LiberationSans-Regular.ttf \
  -fill '#aeaec2' -pointsize 26 \
  -annotate +66+326 'Virtual desktops for Hyprland' \
  -annotate +66+361 'that move all your monitors' \
  -annotate +66+396 'at once.' \
  -fill '#58e1ff' -draw 'rectangle 64,438 218,442' \
  -font /usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf \
  -fill '#7e7e96' -pointsize 20 \
  -annotate +66+494 'no plugin. no hyprpm. no sudo.' \
  -annotate +66+524 'stable public IPC only.' \
  -strip assets/social-preview.png
```

ImageMagick wants font file paths here, not family names; resolve one with
`fc-match -f '%{file}\n' 'JetBrainsMono Nerd Font:bold'`. The accent colour
matches the Hyprland badge at the top of the main README.

Check the result at around 340px wide before uploading, because that is
roughly how a feed renders it. Fine detail does not survive: a full-screen
terminal collapses into a featureless rectangle at that size, while a
couple of distinct windows with some colour still read as two screens.

## `demo.tape` - the CLI recording script

For the command-line surface only, `demo.tape` is a
[VHS](https://github.com/charmbracelet/vhs) script, so the recording is
reproducible and re-renders identically after a CLI change:

```bash
paru -S vhs        # or: go install github.com/charmbracelet/vhs@latest
vhs assets/demo.tape
```

It writes `assets/cli.gif`. This is a nice secondary asset for the
scripting section. It is not a substitute for the screen capture above.
