#!/usr/bin/env python3
"""Draws docs/status.svg, the picture of how much of Picasso Suite is made, and puts the same
numbers in the README's Status section.

    python3 scripts/status.py          # rewrites docs/status.svg and the README block
    python3 scripts/status.py --check  # exits 1 when either is out of date (for CI)

Two parts, each a treemap with one cell per item, green where it is made and grey where it is not:

* Menu entries: `Menu-Status.md`, which `picasso-suite --menu-status` writes from the table the
  menus are drawn from (a cell for every entry, grouped by menu).
* Roadmap items: `ROADMAP.md`, its `- [x]` Done and `- [ ]` To do lines (a cell for every
  item, grouped by the area before the first colon). To-do items are bigger than done ones, so
  this share looks better than the work left.

Only the standard library is used.
"""
import html
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "docs" / "status.svg"
README = ROOT / "README.md"
START, END = "<!-- status:start -->", "<!-- status:end -->"

DONE, TODO = "#2ea043", "#3a3d44"
DONE_LINE, TODO_LINE = "#1b7f32", "#2a2c31"
BACK, TEXT, DIM = "#16181d", "#e6e6ea", "#9a9aa2"


def read(path):
    return path.read_bytes().decode("utf-8").replace("\r\n", "\n")


def menu_groups():
    """[(menu, [done?, ...])] from Menu-Status.md."""
    groups, name = [], None
    for line in read(ROOT / "Menu-Status.md").split("\n"):
        if line.startswith("## "):
            name = line[3:].strip()
            groups.append((name, []))
        elif name and line.startswith("|") and not line.startswith("|---"):
            cells = [c.strip() for c in line.strip("|").split("|")]
            if cells[-1] in ("works", "planned"):
                groups[-1][1].append(cells[-1] == "works")
    return [(n, items) for n, items in groups if items]


def roadmap_groups():
    """[(area, [done?, ...])] from ROADMAP.md, areas in order of first appearance."""
    order, areas = [], {}
    for line in read(ROOT / "ROADMAP.md").split("\n"):
        m = re.match(r"- \[([ x])\] (.*)", line)
        if not m:
            continue
        head = m.group(2).split(":", 1)[0]
        area = head if ":" in m.group(2) and len(head) <= 24 else "Other"
        area = {"Colour": "Color", "Color page": "Color", "Cut page": "Cut"}.get(area, area)
        if area not in areas:
            areas[area] = []
            order.append(area)
        areas[area].append(m.group(1) == "x")
    return [(a, areas[a]) for a in order]


def squarify(sizes, x, y, w, h):
    """Rectangles (x, y, w, h) for `sizes` (largest first) filling the box: squarified treemap."""
    total = float(sum(sizes))
    if not sizes or total <= 0:
        return []
    scale = w * h / total
    areas = [s * scale for s in sizes]
    out, i = [], 0
    while i < len(areas):
        side = min(w, h)
        row, best = [], float("inf")
        while i + len(row) < len(areas):
            trial = row + [areas[i + len(row)]]
            s = sum(trial)
            worst = max(max(side * side * a / (s * s), s * s / (side * side * a)) for a in trial)
            if row and worst > best:
                break
            row, best = trial, worst
        s = sum(row)
        thick = s / side
        pos = 0.0
        for a in row:
            length = a / thick
            if w >= h:
                out.append((x, y + pos, thick, length))
            else:
                out.append((x + pos, y, length, thick))
            pos += length
        if w >= h:
            x, w = x + thick, w - thick
        else:
            y, h = y + thick, h - thick
        i += len(row)
    return out


def treemap(groups, x, y, w, h):
    """SVG for the groups in the box: a cell for each item, a label on each group."""
    parts = []
    ordered = sorted(groups, key=lambda g: -len(g[1]))
    for (name, items), (gx, gy, gw, gh) in zip(
        ordered, squarify([len(g[1]) for g in ordered], x, y, w, h)
    ):
        # Within a group the done cells come first, so the green sits together.
        items = sorted(items, key=lambda d: not d)
        for done, (cx, cy, cw, ch) in zip(items, squarify([1] * len(items), gx, gy, gw, gh)):
            parts.append(
                f'<rect x="{cx:.1f}" y="{cy:.1f}" width="{cw:.1f}" height="{ch:.1f}" '
                f'fill="{DONE if done else TODO}" stroke="{DONE_LINE if done else TODO_LINE}" '
                f'stroke-width="1"/>'
            )
        parts.append(
            f'<rect x="{gx:.1f}" y="{gy:.1f}" width="{gw:.1f}" height="{gh:.1f}" fill="none" '
            f'stroke="{BACK}" stroke-width="2"/>'
        )
        # The name, when the group is big enough to hold it.
        if gw >= 7 * len(name) + 8 and gh >= 18:
            parts.append(
                f'<text x="{gx + 5:.1f}" y="{gy + 13:.1f}" font-size="11" fill="#fff" '
                f'stroke="{BACK}" stroke-width="2.5" paint-order="stroke">{html.escape(name)}</text>'
            )
    return "\n".join(parts)


def share(groups):
    done = sum(sum(items) for _, items in groups)
    total = sum(len(items) for _, items in groups)
    return done, total, 100.0 * done / total if total else 0.0


def badge(x, y, label, value, colour):
    lw, vw = 7 * len(label) + 16, 7 * len(value) + 14
    return (
        f'<g transform="translate({x},{y})" font-size="11" font-weight="600">'
        f'<rect width="{lw}" height="20" rx="3" fill="#555"/>'
        f'<rect x="{lw}" width="{vw}" height="20" rx="3" fill="{colour}"/>'
        f'<rect x="{lw - 3}" width="6" height="20" fill="{colour}"/>'
        f'<text x="8" y="14" fill="#fff">{html.escape(label)}</text>'
        f'<text x="{lw + 7}" y="14" fill="#fff">{html.escape(value)}</text></g>'
    )


def colour_for(pct):
    return "#4c1" if pct >= 90 else "#97ca00" if pct >= 75 else "#dfb317" if pct >= 40 else "#e05d44"


def build():
    menus, roads = menu_groups(), roadmap_groups()
    (md, mt, mp), (rd, rt, rp) = share(menus), share(roads)
    width, height, pad, gap, top = 900, 420, 12, 16, 78
    box_w = (width - 2 * pad - gap) / 2
    box_h = height - top - pad
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="Segoe UI, Helvetica, Arial, sans-serif" '
        f'role="img" aria-label="Menu entries {mp:.2f}% done ({md} of {mt}); roadmap items '
        f'{rp:.2f}% done ({rd} of {rt})">',
        f'<rect width="{width}" height="{height}" rx="8" fill="{BACK}"/>',
        badge(pad, pad, "menu entries", f"{mp:.2f}%", colour_for(mp)),
        badge(pad + 150, pad, "roadmap", f"{rp:.2f}%", colour_for(rp)),
        f'<text x="{pad}" y="{top - 8}" font-size="13" fill="{TEXT}">Menu entries: '
        f"{mp:.2f}% ({md}/{mt})</text>",
        f'<text x="{pad + box_w + gap}" y="{top - 8}" font-size="13" fill="{TEXT}">Roadmap '
        f"items: {rp:.2f}% ({rd}/{rt})</text>",
        treemap(menus, pad, top, box_w, box_h),
        treemap(roads, pad + box_w + gap, top, box_w, box_h),
        "</svg>",
    ]
    block = (
        f"{START}\n"
        f"![Status: {mp:.2f}% of the menu entries work, {rp:.2f}% of the roadmap is done]"
        f"(docs/status.svg)\n\n"
        f"| | Done | Total | |\n|---|---|---|---|\n"
        f"| Menu entries | {md} | {mt} | {mp:.2f}% |\n"
        f"| Roadmap items* | {rd} | {rt} | {rp:.2f}% |\n\n"
        f"Green is made, grey is not. Menu entries: every entry of every menu "
        f"([`Menu-Status.md`](Menu-Status.md)); the rest are shown disabled. "
        f"\\* Roadmap items ([`ROADMAP.md`](ROADMAP.md)): the to-do items are bigger than the "
        f"done ones, so this share looks better than the work left. "
        f"Made by `python3 scripts/status.py`.\n"
        f"{END}"
    )
    return "\n".join(svg) + "\n", block


def main():
    svg, block = build()
    text = read(README)
    if START in text:
        a, b = text.index(START), text.index(END) + len(END)
        new = text[:a] + block + text[b:]
    else:
        marker = "## Screenshots"
        new = text.replace(marker, "## Status\n\n" + block + "\n\n" + marker, 1)
    # The README keeps its line endings.
    crlf = "\r\n" in README.read_bytes().decode("utf-8")
    new_bytes = (new.replace("\n", "\r\n") if crlf else new).encode("utf-8")
    old_svg = SVG.read_bytes() if SVG.exists() else b""
    if "--check" in sys.argv:
        stale = old_svg != svg.encode("utf-8") or README.read_bytes() != new_bytes
        print("status is out of date: run python3 scripts/status.py" if stale else "status is current")
        sys.exit(1 if stale else 0)
    SVG.write_bytes(svg.encode("utf-8"))
    README.write_bytes(new_bytes)
    print(f"wrote {SVG.relative_to(ROOT)} and the README Status section")


if __name__ == "__main__":
    main()
