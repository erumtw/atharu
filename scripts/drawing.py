"""Build a note drawing an agent can write with `atharu note`: one markdown line
`![Drawing](data:image/svg+xml;base64,…)` whose SVG renders as-is and carries an editable Excalidraw scene.

    import sys; sys.path.insert(0, "scripts"); from drawing import Drawing
    d = Drawing()
    a = d.box(40, 80, 200, 56, "Draft"); b = d.box(40, 180, 200, 56, "Review", d.BLUE, "#e7f5ff")
    d.arrow(a, b)
    print(d.line())          # paste on its own line in the note body

`python3 scripts/drawing.py [out.svg]` prints the example below (and writes the SVG to look at, e.g. `rsvg-convert`).
"""
import base64
import json
import sys
from html import escape


class Drawing:
    GREY, BLUE, GREEN, ORANGE, RED = "#868e96", "#1971c2", "#2f9e44", "#e8590c", "#e03131"

    def __init__(self, font=16):
        self.font, self.els, self.svg, self.n = font, [], [], 0

    def _el(self, kind, x, y, w, h, stroke="#1e1e1e", bg="transparent", dashed=False, **kw):
        self.n += 1
        # Excalidraw's restore fills any field left out; these are the ones that change how it looks.
        el = {"type": kind, "id": f"{kind[:3]}{self.n}", "x": x, "y": y, "width": w, "height": h, "angle": 0,
              "strokeColor": stroke, "backgroundColor": bg, "fillStyle": "solid", "strokeWidth": 2,
              "strokeStyle": "dashed" if dashed else "solid", "roughness": 0, "opacity": 100, "groupIds": [],
              "frameId": None, "roundness": None, "seed": self.n * 7919, "version": 1,
              "versionNonce": self.n * 104729, "isDeleted": False, "boundElements": [], "updated": 1,
              "link": None, "locked": False, **kw}
        self.els.append(el)
        return el

    def text(self, x, y, w, s, color="#1e1e1e", size=None, align="left", container=None):
        size = size or self.font
        lines = s.split("\n")
        t = self._el("text", x, y, w, len(lines) * size * 1.25, stroke=color, text=s, originalText=s,
                     fontSize=size, fontFamily=5, textAlign=align, containerId=container, autoResize=True,
                     verticalAlign="middle" if container else "top", lineHeight=1.25)
        anchor, ax = ("middle", x + w / 2) if align == "center" else ("start", x)
        for i, line in enumerate(lines):
            self.svg.append(f'<text x="{ax}" y="{y + size * (i + 1)}" font-size="{size}" fill="{color}" '
                            f'text-anchor="{anchor}">{escape(line)}</text>')
        return t

    def box(self, x, y, w, h, label, stroke="#1e1e1e", bg="#ffffff", dashed=False, color="#1e1e1e"):
        r = self._el("rectangle", x, y, w, h, stroke=stroke, bg=bg, dashed=dashed, roundness={"type": 3})
        dash = ' stroke-dasharray="8 6"' if dashed else ""
        self.svg.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="{bg}" '
                        f'stroke="{stroke}" stroke-width="2"{dash}/>')
        lines = label.count("\n") + 1
        t = self.text(x + 8, y + (h - lines * self.font * 1.25) / 2, w - 16, label, color, align="center",
                      container=r["id"])
        r["boundElements"].append({"type": "text", "id": t["id"]})
        return r

    def arrow(self, a, b, color="#1e1e1e", dashed=False):
        """Bound arrow from box a to box b: down when b sits below a, else sideways."""
        if b["y"] >= a["y"] + a["height"]:
            x1, y1 = a["x"] + a["width"] / 2, a["y"] + a["height"]
            x2, y2 = b["x"] + b["width"] / 2, b["y"]
        else:
            right = b["x"] >= a["x"] + a["width"]
            x1, y1 = (a["x"] + a["width"] if right else a["x"]), a["y"] + a["height"] / 2
            x2, y2 = (b["x"] if right else b["x"] + b["width"]), b["y"] + b["height"] / 2
        dx, dy = x2 - x1, y2 - y1
        ar = self._el("arrow", x1, y1, abs(dx), abs(dy), stroke=color, dashed=dashed, points=[[0, 0], [dx, dy]],
                      lastCommittedPoint=None, startBinding={"elementId": a["id"], "focus": 0, "gap": 1},
                      endBinding={"elementId": b["id"], "focus": 0, "gap": 1}, startArrowhead=None,
                      endArrowhead="arrow", elbowed=False, roundness={"type": 2})
        for box in (a, b):
            box["boundElements"].append({"type": "arrow", "id": ar["id"]})
        # Arrowhead: two 10px strokes back from the tip, either side of the line.
        ln = (dx * dx + dy * dy) ** 0.5 or 1
        ux, uy = dx / ln, dy / ln
        bx, by = x2 - ux * 10, y2 - uy * 10
        dash = ' stroke-dasharray="8 6"' if dashed else ""
        self.svg.append(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="2"{dash}/>'
                        f'<path d="M{bx - uy * 6} {by + ux * 6} L{x2} {y2} L{bx + uy * 6} {by - ux * 6}" '
                        f'fill="none" stroke="{color}" stroke-width="2"/>')
        return ar

    def to_svg(self, pad=20):
        xs = [e["x"] for e in self.els] + [e["x"] + e["width"] for e in self.els]
        ys = [e["y"] for e in self.els] + [e["y"] + e["height"] for e in self.els]
        x0, y0 = min(xs) - pad, min(ys) - pad
        w, h = max(xs) - x0 + pad, max(ys) - y0 + pad
        scene = {"type": "excalidraw", "version": 2, "source": "atharu", "elements": self.els,
                 "appState": {"viewBackgroundColor": "#ffffff", "gridSize": None}, "files": {}}
        # payload-version:1 = plain base64 of the UTF-8 scene JSON: the un-compressed form Excalidraw's
        # decodeSvgBase64Payload accepts, so no pako/deflate here (checked against @excalidraw/excalidraw 0.18.1).
        payload = base64.b64encode(json.dumps(scene, ensure_ascii=False).encode()).decode()
        return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0} {y0} {w} {h}" width="{w}" height="{h}">'
                f'<!-- svg-source:excalidraw --><metadata><!-- payload-type:application/vnd.excalidraw+json -->'
                f'<!-- payload-version:1 --><!-- payload-start -->{payload}<!-- payload-end --></metadata>'
                f'<g font-family="Excalifont, Helvetica Neue, Helvetica, Arial, sans-serif">{"".join(self.svg)}</g></svg>')

    def line(self):
        """The note-body line; editor.ts DRAWING_LINE and markdown.rs both expect exactly this shape."""
        return "![Drawing](data:image/svg+xml;base64," + base64.b64encode(self.to_svg().encode()).decode() + ")"


if __name__ == "__main__":
    d = Drawing()
    d.text(40, 0, 400, "OPS-101 rollout", size=24)
    a = d.box(40, 60, 200, 56, "Draft")
    b = d.box(40, 160, 200, 56, "Review", d.BLUE, "#e7f5ff")
    c = d.box(300, 160, 200, 56, "Shipped", d.GREEN, "#ebfbee")
    d.arrow(a, b)
    d.arrow(b, c, d.GREY, dashed=True)
    if len(sys.argv) > 1:
        open(sys.argv[1], "w").write(d.to_svg())
    print(d.line())
