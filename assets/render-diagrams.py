#!/usr/bin/env python3
"""Render the two README diagrams as light/dark SVG pairs.

The README used to draw these with mermaid fences. GitHub scales any
diagram down to the content column, which on a phone is about 358px wide,
so what matters is not the absolute font size but its ratio to the
viewBox width. Mermaid's defaults worked out to roughly 1/60th, landing
the labels around 7px and unreadable.

Everything here is laid out against that ratio instead: no text is
smaller than viewBox/32 (about 11px on a phone, 24px on a desktop
README), and the desk titles are viewBox/25 (about 14px on a phone).
That is the whole reason these are hand-laid rather than generated.

Run from the repo root:

    python3 assets/render-diagrams.py

It writes four files and needs nothing installed.
"""

import pathlib

OUT = pathlib.Path(__file__).resolve().parent

# Dark is GitHub Primer dark, the same ramp assets/social-preview.png uses,
# so the card and the README diagrams stay one family. Light is Primer
# light. Both diagrams are all-grey: nothing is singled out by colour, so
# the only thing a reader's eye is drawn to is the shape of the nesting.
THEMES = {
    "light": {
        "page": "#ffffff",
        "pageBorder": "#d1d9e0",
        "row": "#f6f8fa",
        "rowBorder": "#d1d9e0",
        "card": "#ffffff",
        "cardBorder": "#d1d9e0",
        "text": "#1f2328",
        "muted": "#59636e",
        "chip": "#eaeef2",
        "chipBorder": "#d1d9e0",
        "chipText": "#1f2328",
    },
    "dark": {
        "page": "#0d1117",
        "pageBorder": "#30363d",
        "row": "#161b22",
        "rowBorder": "#30363d",
        "card": "#21262d",
        "cardBorder": "#3d444d",
        "text": "#e6edf3",
        "muted": "#9198a1",
        "chip": "#21262d",
        "chipBorder": "#3d444d",
        "chipText": "#e6edf3",
    },
}

STYLE = """
    .sans { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI",
            "Noto Sans", Helvetica, Arial, sans-serif; }
    .mono { font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo,
            Consolas, "Liberation Mono", monospace; }
"""


def head_svg(width, height, title, desc):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}"'
        f' width="{width}" height="{height}" role="img"'
        ' aria-labelledby="title desc">\n'
        f"  <title id=\"title\">{title}</title>\n"
        f"  <desc id=\"desc\">{desc}</desc>\n"
        f"  <style>{STYLE}  </style>\n"
    )


def rect(x, y, w, h, fill, stroke, rx=10, sw=1.5, dash=None):
    dashed = f' stroke-dasharray="{dash}"' if dash else ""
    return (
        f'  <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}"'
        f' fill="{fill}" stroke="{stroke}" stroke-width="{sw}"{dashed}/>\n'
    )


def text(x, y, s, fill, size, cls="sans", weight=400, anchor="middle"):
    return (
        f'  <text x="{x}" y="{y}" class="{cls}" font-size="{size}"'
        f' font-weight="{weight}" fill="{fill}" text-anchor="{anchor}">{s}</text>\n'
    )


# ---------------------------------------------------------------- diagram 1

def desks(t):
    """One desk is a container holding every monitor, so one keypress moves
    the whole container. Row 4 is the placeholder that says there are as
    many desks as you want, not just three."""
    w, h = 760, 648
    box_x, box_w = 18, 724
    chip_x, chip_w = 34, 150
    lap_x, lap_w = 198, 232
    ext_x, ext_w = 444, 282
    box_h, box_gap, box_top = 132, 14, 18
    rows = ["1", "2", "3", "N"]

    s = head_svg(
        w, h,
        "hyprdesk desks",
        "Four desks stacked as rows. Each row is one desk, holding the "
        "laptop and the external monitor together, so SUPER+2 switches "
        "both screens to desk 2 at once. The fourth row is desk N, "
        "standing for as many desks as you want.",
    )
    s += rect(1, 1, w - 2, h - 2, t["page"], t["pageBorder"], rx=14)

    for i, n in enumerate(rows):
        more = n == "N"  # the "and so on" row
        top = box_top + i * (box_h + box_gap)
        mid = top + 84

        s += rect(box_x, top, box_w, box_h, t["row"], t["rowBorder"],
                  rx=14, dash="9 7" if more else None)
        s += text(box_x + box_w / 2, top + 39, f"desk {n}",
                  t["text"], 30, weight=700)

        # The key floats unboxed, like the desk title: only the monitors
        # get a box, because only they are things a desk contains.
        s += text(chip_x + chip_w / 2, mid + 9, f"SUPER+{n}",
                  t["text"], 25, cls="mono", weight=600)

        for cx, cw, label in ((lap_x, lap_w, "Laptop"),
                              (ext_x, ext_w, "External monitor")):
            s += rect(cx, mid - 32, cw, 64, t["card"], t["cardBorder"], rx=10)
            s += text(cx + cw / 2, mid + 9, label, t["text"], 24, weight=500)

    s += text(w / 2, 622, "One keypress switches every screen to the same desk.",
              t["muted"], 24)
    return s + "</svg>\n"


# ---------------------------------------------------------------- diagram 2

def stock(t):
    """Stock Hyprland: each number key is stuck on one screen."""
    w, h = 760, 400
    card_y, card_h = 58, 266
    chip_w, chip_h, chip_gap = 180, 58, 18

    s = head_svg(
        w, h,
        "Stock Hyprland workspaces",
        "With stock Hyprland, SUPER+1 and SUPER+4 belong to the laptop "
        "while SUPER+2, SUPER+3 and SUPER+5 belong to the external "
        "monitor, so each key changes only one screen.",
    )
    s += rect(1, 1, w - 2, h - 2, t["page"], t["pageBorder"], rx=14)

    for cx, cw, label, keys in (
        (18, 312, "Laptop", (1, 4)),
        (350, 392, "External monitor", (2, 3, 5)),
    ):
        mid_x = cx + cw / 2
        s += text(mid_x, 44, label, t["muted"], 24)
        s += rect(cx, card_y, cw, card_h, t["card"], t["cardBorder"], rx=14)

        span = len(keys) * chip_h + (len(keys) - 1) * chip_gap
        top = card_y + card_h / 2 - span / 2
        for n, key in enumerate(keys):
            y = top + n * (chip_h + chip_gap)
            s += rect(mid_x - chip_w / 2, y, chip_w, chip_h,
                      t["chip"], t["chipBorder"], rx=10)
            s += text(mid_x, y + chip_h / 2 + 9, f"SUPER+{key}",
                      t["chipText"], 25, cls="mono", weight=600)

    s += text(w / 2, 366, "Each SUPER+N is stuck on one screen.", t["muted"], 24)
    return s + "</svg>\n"


for name, draw in (("desks", desks), ("stock-workspaces", stock)):
    for theme, palette in THEMES.items():
        path = OUT / f"{name}-{theme}.svg"
        path.write_text(draw(palette))
        print(f"wrote {path.relative_to(OUT.parent)} ({path.stat().st_size} bytes)")
