"""Render the terrain example's actual OBJ quads; optional dev dependency: Pillow.

Run from the package root after generating terrain.obj:
    python docs/render_preview.py terrain.obj docs/terrain.png
This deliberately reads the example's four-vertices-per-quad OBJ layout.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def render(source, destination):
    vertices = []
    materials = []
    faces = 0
    for line in Path(source).read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "v":
            vertices.append(tuple(map(float, parts[1:4])))
        elif parts[0] == "g":
            materials.append(int(parts[1].removeprefix("material_")))
        elif parts[0] == "f":
            faces += 1
    assert len(vertices) == 4 * len(materials)
    assert faces == 2 * len(materials)
    polygons = []
    for i, material in enumerate(materials):
        quad = vertices[4 * i:4 * i + 4]
        a = [quad[1][j] - quad[0][j] for j in range(3)]
        b = [quad[2][j] - quad[0][j] for j in range(3)]
        normal = (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
        if sum(normal) <= 0:  # Orthographic camera points toward (-1,-1,-1).
            continue
        axis = max(range(3), key=lambda j: abs(normal[j]))
        shade = (0.72, 1.0, 0.86)[axis]
        color = (100, 187, 112) if material == 2 else (160, 143, 116)
        color = tuple(round(channel * shade) for channel in color)
        projected = [(x - z, (x + z) * 0.5 - y) for x, y, z in quad]
        depth = sum(sum(p) for p in quad) / 4
        polygons.append((depth, projected, color))
    width, height, aa = 1100, 850, 2
    image = Image.new("RGB", (width * aa, height * aa), (19, 26, 37))
    draw = ImageDraw.Draw(image)
    all_points = [p for _, polygon, _ in polygons for p in polygon]
    min_x = min(p[0] for p in all_points)
    max_x = max(p[0] for p in all_points)
    min_y = min(p[1] for p in all_points)
    max_y = max(p[1] for p in all_points)
    scale = min((width - 120) / (max_x - min_x), 585 / (max_y - min_y))
    offset_x = width / 2 - (min_x + max_x) * scale / 2
    offset_y = 155 - min_y * scale
    for _, points, color in sorted(polygons, key=lambda row: row[0]):
        points = [((offset_x + x * scale) * aa, (offset_y + y * scale) * aa) for x, y in points]
        draw.polygon(points, fill=color)
        draw.line(points + points[:1], fill=tuple(round(v * 0.6) for v in color), width=aa)
    def font(size):
        for name in ("DejaVuSans.ttf", "C:/Windows/Fonts/segoeui.ttf"):
            try:
                return ImageFont.truetype(name, size * aa)
            except OSError:
                pass
        return ImageFont.load_default(size=size * aa)
    draw.text((48 * aa, 30 * aa), "voxel-forge / actual OBJ geometry", fill=(231, 238, 245), font=font(29))
    draw.text((48 * aa, 78 * aa), f"16 x 16 x 16 section | {len(materials):,} quads | {faces:,} triangles", fill=(151, 173, 194), font=font(19))
    draw.text((48 * aa, 785 * aa), "Synthetic terrain - CPU mesh preview, not a game screenshot", fill=(151, 173, 194), font=font(18))
    image.resize((width, height), Image.Resampling.LANCZOS).save(destination)
    print(f"Rendered {len(polygons)} camera-facing quads from {len(materials)} OBJ quads: {destination}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: render_preview.py INPUT.obj OUTPUT.png")
    render(sys.argv[1], sys.argv[2])
