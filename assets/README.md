# Assets

`demo.gif` is a real capture of a live two-monitor session;
`social-preview.png` is drawn from the same diagram the main README uses.
This file records how each one was made, so any of them can be regenerated
after a UI change without rediscovering the mechanics.

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

It is **1280x640**, the size GitHub documents. The card carries the same
desks diagram as the main README rather than screenshots, so the two stay
in step: when the README diagram changes, paste the new source into step 1
and rebuild.

### The palette

Every colour on the card is a [GitHub Primer
dark](https://primer.style/foundations/color/overview) token, the same
design system github.com itself is painted with. That is not decoration:
the card is displayed inside GitHub's own chrome and in link unfurls that
imitate it, so borrowing GitHub's neutrals makes it sit in the page
instead of fighting it. The practical effect is that every grey is drawn
from one cool ramp, with no second hue mixed in anywhere.

| Role | Token | Hex |
|------|-------|-----|
| Page | `canvas.inset` | `#0c111a` to `#070a0f` |
| Desk group | `canvas.default` | `#0d1117`, border `#30363d` |
| Monitor box | `canvas.overlay` | `#21262d`, border `#3d444d` |
| Wordmark | `fg.default` | `#f0f6fc` |
| `SUPER+N` | white | `#ffffff` |
| Tagline | `fg.muted` | `#b1bac4` |
| Arrows | `neutral.emphasis` | `#6e7681` |

The diagram sits **directly on the page**, with no container panel behind
it. A panel was tried and removed: at this size it read as a slab with
three boxes stranded on it, and the extra edge competed with the desk
group borders instead of supporting them. The three surfaces stack in
value order on their own - page, desk group barely raised off it, monitor
box raised again - and the group borders do the separating.

Step 1 renders the diagram on a transparent background in that palette.
`themeVariables` covers the boxes and arrows; the cluster labels need the
CSS override, because their colour is not exposed as a theme variable.
`subGraphTitleMargin` is what keeps `SUPER+N` off the top edge of its box.

The trailing `style etc.` line is there because `etc.` is a plain node,
so it would otherwise take `primaryColor` and render in the monitor box
colour. It stands for the desks that continue past the third, not for a
monitor inside one, so it is painted as a desk group instead. This is the
only line the card's copy of the diagram adds to the README's - the graph
itself is identical, and must stay that way.

```bash
cat > /tmp/card.mmd <<'MMD'
%%{init: {'theme':'base','themeVariables':{
'fontFamily':'JetBrains Mono, DejaVu Sans Mono, monospace',
'fontSize':'17px',
'primaryColor':'#21262d',
'primaryTextColor':'#e6edf3',
'primaryBorderColor':'#3d444d',
'lineColor':'#6e7681',
'clusterBkg':'#0d1117',
'clusterBorder':'#30363d'
},'flowchart':{'subGraphTitleMargin':{'top':10,'bottom':14},'padding':18}}}%%
flowchart LR
    subgraph s1["SUPER+1"]
        direction LR
        s1a["External Monitor<br/>desk 1"]
        s1b["Laptop<br/>desk 1"]
    end
    subgraph s2["SUPER+2"]
        direction LR
        s2a["External Monitor<br/>desk 2"]
        s2b["Laptop<br/>desk 2"]
    end
    subgraph s3["SUPER+3"]
        direction LR
        s3a["External Monitor<br/>desk 3"]
        s3b["Laptop<br/>desk 3"]
    end
    s1 --> s2 --> s3 --> etc.
    style etc. fill:#0d1117,stroke:#30363d
MMD

cat > /tmp/card.css <<'CSS'
.nodeLabel, .nodeLabel p { color: #e6edf3 !important; }
.cluster-label .nodeLabel, .cluster-label .nodeLabel p,
.cluster-label p, .cluster .label { color: #ffffff !important; font-weight: 700 !important; }
CSS

printf '{"args":["--no-sandbox","--disable-gpu"]}\n' > /tmp/puppeteer.json

PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium \
  npx @mermaid-js/mermaid-cli -i /tmp/card.mmd -o /tmp/diag.png \
    -p /tmp/puppeteer.json -C /tmp/card.css -b transparent -s 3
magick /tmp/diag.png -trim +repage -resize 1152x /tmp/diag-fit.png
```

The `-trim` matters. Mermaid leaves a few pixels of transparent margin,
and trimming first is what lets the diagram land on the 64px margin by
its actual content rather than by its bounding box. 1152 is that margin
doubled and subtracted from the width, so the diagram spans the full
column and sets the margin every other element is placed against.

Step 2 composites the card. The layout is three centred blocks stacked
down the page - wordmark, diagram, tagline - and every gap between and
around them is the same 64px the diagram already uses left and right.

```bash
magick -size 1280x640 gradient:'#0c111a-#070a0f' \
  \( /tmp/diag-fit.png \) -gravity northwest -geometry +64+181 -composite \
  -gravity north \
  -font /usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf \
  -fill '#f0f6fc' -pointsize 58 -annotate +0+64 'hyprdesk' \
  -gravity north \
  -font /usr/share/fonts/liberation/LiberationSans-Regular.ttf \
  -fill '#b1bac4' -pointsize 23 \
  -annotate +0+553 'Virtual desktops for Hyprland that behave like workspaces.' \
  -strip assets/social-preview.png
```

Those three y offsets are not free choices, they are what makes the
margins come out equal. The ink measures 54px for the wordmark, 308 for
the diagram, 23 for the tagline, which is 385 of the 640 available. The
remaining 255 splits four ways - top margin, two gaps, bottom margin - so
each is about 64. Hence `y=64`, then `64+54+63 = 181`, then
`181+308+64 = 553`, with 64px left under the tagline.

That arithmetic only works because of one non-obvious ImageMagick rule:
**once `-gravity` is set, the `-annotate +x+y` offset places the top of
the text, not its baseline.** Reasoning about baselines and descenders
under gravity produces numbers that are simply wrong. Measure the ink
instead - render a string alone on transparency with the same gravity and
offset the card uses, `-trim`, and read the geometry back:

```bash
magick -size 1280x640 xc:none -gravity north \
  -font <font> -fill white -pointsize 58 \
  -annotate +0+64 'hyprdesk' -trim info:
```

That reports `273x54 ... +504+64`: ink 54px tall starting exactly at the
requested y, and horizontally centred to within half a pixel. Re-measure
after any point-size or font change and redo the split above; the three
ink heights are what the answer is made of.

Three things here are worth not rediscovering:

- **Let mermaid-cli write the PNG.** Rendering to SVG and converting with
  `magick` yields empty boxes, because mermaid puts every label in a
  `<foreignObject>` and ImageMagick's SVG renderer drops those entirely.
- **`-s 3` is what sets the resolution.** `-w` only sizes the browser
  viewport; the output still comes out at the diagram's natural size, so
  text ends up soft once scaled to card width.
- **ImageMagick wants font file paths, not family names.** Resolve one with
  `fc-match -f '%{file}\n' 'JetBrainsMono Nerd Font:bold'`.

Check the result at around 340px wide before uploading, because that is
roughly how a feed renders it. The wordmark, the tagline, and the shape
of the diagram all survive; the labels inside the boxes do not, which is
the trade for showing the mechanism rather than a screenshot. An earlier
draft also carried a smaller grey line along the bottom, and dropping it
is what left the tagline enough room to stay readable at that size.

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
