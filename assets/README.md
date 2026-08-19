# Assets

`demo.gif` in this directory is a **placeholder**. Replace it with a real
capture and the README picks it up with no edits, since it references it
by that exact name.

## `demo.gif` - the one that sells the project

This is the single most valuable asset in the repo. hyprdesk is a
multi-monitor tool, so a terminal recording cannot show what it does: the
point is *two screens changing at the same time*. It needs a real screen
capture.

**What to capture,** in about 10 to 15 seconds:

1. Start on desk 1 with something recognisable open on each monitor, for
   example a browser on the external and a terminal on the laptop.
2. Press `SUPER+2`. Both monitors change together. Let it sit for a beat
   so the viewer registers that both moved.
3. Press `SUPER+3`, then `SUPER+1` to come back.
4. Optionally finish by dragging a window with `SUPER+SHIFT+2`.

Keep the mouse still. The whole argument is that you never had to move it.

**Recording it on Omarchy:**

```bash
omarchy capture screenrecord          # pick the region or all outputs
omarchy capture screenrecord --stop   # or the stop keybind
```

**Converting to a GIF** that GitHub will actually load. Keep it under
about 10 MB, and under 5 MB if you can:

```bash
ffmpeg -i recording.mp4 -vf "fps=15,scale=1200:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" -loop 0 assets/demo.gif
```

If it comes out too large, drop `fps` to 12 or `scale` to 1000.

An `.mp4` is also fine and often looks better. GitHub plays uploaded video
in READMEs, but only when the file is attached through the web UI rather
than committed, so a committed GIF is the more portable choice.

## `social-preview.png` - optional but high value

GitHub shows this image whenever the repo is linked on Reddit, Discord, or
anywhere else with link previews, which is exactly where a Hyprland tool
gets found. It is **not** committed to the repo; you upload it under
Settings, then General, then Social preview.

Make it **1280x640**, with the name `hyprdesk` and the one-line pitch
large enough to read as a thumbnail. A cropped frame from the demo
recording with the title over it works well.

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
