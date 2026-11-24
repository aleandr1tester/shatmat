# -*- coding: utf-8 -*-
"""Иконка ShatMat: белая шашка на фоне, чёрный ферзь на переднем плане.

Скрипт самодостаточный (только Pillow): разбирает полигоны из
pieces/black_queen.svg, рисует иконку в нескольких размерах, собирает
assets/icon.ico и assets/app.res (ресурс для линковки иконки в exe).
"""
import io
import os
import re
import struct

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SVG_QUEEN = os.path.join(ROOT, 'pieces', 'black_queen.svg')
ASSETS = os.path.join(ROOT, 'assets')
OUT_ICO = os.path.join(ASSETS, 'icon.ico')
OUT_PNG = os.path.join(ASSETS, 'icon-256.png')
OUT_PREVIEW = os.path.join(ASSETS, 'icon-preview.jpg')

SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def svg_polygons(path):
    """Полигоны из SVG: команды M/L/Z (у ферзя кривых нет)."""
    text = io.open(path, encoding='utf-8').read()
    out = []
    for tag in re.finditer(r'<path\b([^>]*)/>', text):
        attrs = dict(re.findall(r'([a-zA-Z-]+)="([^"]*)"', tag.group(1)))
        d = attrs.get('d', '')
        fill = attrs.get('fill', '#000000')
        stroke = attrs.get('stroke', 'none')
        stroke_w = float(attrs.get('stroke-width', '0') or 0)
        tokens = re.findall(r'[MLZmlz]|-?\d+(?:\.\d+)?', d)
        polys = []
        cur = []
        i = 0
        while i < len(tokens):
            t = tokens[i]
            if t in 'Mm':
                if cur:
                    polys.append(cur)
                cur = []
                i += 1
                while i + 1 < len(tokens) and tokens[i] not in 'MLZmlz':
                    cur.append((float(tokens[i]), float(tokens[i + 1])))
                    i += 2
            elif t in 'Ll':
                i += 1
                while i + 1 < len(tokens) and tokens[i] not in 'MLZmlz':
                    cur.append((float(tokens[i]), float(tokens[i + 1])))
                    i += 2
            elif t in 'Zz':
                if cur:
                    polys.append(cur)
                    cur = []
                i += 1
            else:
                i += 1
        if cur:
            polys.append(cur)
        if polys:
            out.append((polys, fill, stroke, stroke_w))
    return out


def hex_rgba(s, alpha=255):
    s = s.lstrip('#')
    if len(s) == 3:
        s = ''.join(c * 2 for c in s)
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), alpha)


def render(size, ss=4):
    """Рисует иконку в размере size (с суперсэмплингом ss)."""
    S = size * ss
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Тёмная скруглённая плитка с лёгким свечением сверху.
    radius = int(S * 0.22)
    d.rounded_rectangle([0, 0, S - 1, S - 1],
                        radius=radius, fill=(22, 20, 30, 255))
    for i in range(int(S * 0.5)):
        a = int(26 * (1 - i / (S * 0.5)) ** 2)
        if a <= 0:
            break
        d.rounded_rectangle([0, 0, S - 1, S - 1],
                            radius=radius, outline=(70, 78, 120, a), width=1)

    # Белая круглая шашка.
    cx, cy = S / 2, S * 0.535
    rad = S * 0.365
    disc = [cx - rad, cy - rad, cx + rad, cy + rad]
    # мягкая тень под шашкой
    shadow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).ellipse(
        [cx - rad * 1.02, cy - rad * 0.88 + S * 0.03,
         cx + rad * 1.02, cy + rad * 1.12 + S * 0.03],
        fill=(0, 0, 0, 120))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(S * 0.02)))
    d = ImageDraw.Draw(img)
    d.ellipse(disc, fill=(240, 233, 217, 255))
    d.ellipse(disc, outline=(203, 191, 168, 255), width=max(1, int(S * 0.010)))
    # ободок шашки
    inner = rad * 0.80
    d.ellipse([cx - inner, cy - inner, cx + inner, cy + inner],
              outline=(224, 214, 195, 255), width=max(1, int(S * 0.007)))
    # блик
    d.ellipse([cx - rad * 0.55, cy - rad * 0.72, cx - rad * 0.02, cy - rad * 0.28],
              fill=(255, 252, 245, 70))

    # Чёрный ферзь из родного SVG.
    polys = svg_polygons(SVG_QUEEN)
    # видимая область ферзя в координатах SVG
    x0, y0, x1, y1 = 4.0, 9.0, 96.0, 95.0
    qh = S * 0.60
    scale = qh / (y1 - y0)
    qw = (x1 - x0) * scale
    ox = cx - qw / 2 - x0 * scale
    oy = cy + rad * 0.96 - y1 * scale

    def to_pts(poly):
        return [(ox + px * scale, oy + py * scale) for px, py in poly]

    # тень от ферзя на шашке
    qshadow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    qd = ImageDraw.Draw(qshadow)
    for poly_list, fill, stroke, sw in polys:
        for poly in poly_list:
            qd.polygon(to_pts(poly), fill=(0, 0, 0, 110))
    img.alpha_composite(
        qshadow.filter(ImageFilter.GaussianBlur(S * 0.012))
        .transform((S, S), Image.AFFINE, (1, 0, -S * 0.004, 0, 1, -S * 0.006)))
    d = ImageDraw.Draw(img)

    for poly_list, fill, stroke, sw in polys:
        for poly in poly_list:
            pts = to_pts(poly)
            colour = hex_rgba(fill) if fill != 'none' else None
            if colour:
                d.polygon(pts, fill=colour)
            if stroke != 'none' and sw > 0:
                d.line(pts + [pts[0]], fill=hex_rgba(stroke),
                       width=max(1, int(sw * scale)), joint='curve')

    return img.resize((size, size), Image.LANCZOS)


def dib_bytes(img):
    """32-битный DIB (BGRA + AND-маска) — формат изображения для ICO."""
    w, h = img.size
    px = img.load()
    rows = []
    for y in range(h - 1, -1, -1):
        row = bytearray()
        for x in range(w):
            r, g, b, a = px[x, y]
            row += bytes((b, g, r, a))
        rows.append(bytes(row))
    xor = b''.join(rows)
    mask_row = ((w + 31) // 32) * 4
    mask = b'\x00' * (mask_row * h)
    header = struct.pack('<IiiHHIIiiII', 40, w, h * 2, 1, 32, 0,
                         len(xor) + len(mask), 0, 0, 0, 0)
    return header + xor + mask


def png_bytes(img):
    buf = io.BytesIO()
    img.save(buf, format='PNG', optimize=True)
    return buf.getvalue()


def build_ico(images):
    """Собирает ICO: мелкие размеры — DIB, 128/256 — PNG."""
    blobs = []
    for size, img in images:
        data = png_bytes(img) if size >= 128 else dib_bytes(img)
        blobs.append((size, data))
    head = struct.pack('<HHH', 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    entries = b''
    data = b''
    for size, blob in blobs:
        dim = 0 if size >= 256 else size
        entries += struct.pack('<BBBBHHII', dim, dim, 0, 0, 1, 32,
                              len(blob), offset + len(data))
        data += blob
    return head + entries + data, blobs


def main():
    os.makedirs(ASSETS, exist_ok=True)
    images = [(s, render(s)) for s in SIZES]
    ico, blobs = build_ico(images)
    io.open(OUT_ICO, 'wb').write(ico)
    images[-1][1].save(OUT_PNG)
    images[-1][1].convert('RGB').save(OUT_PREVIEW, 'JPEG', quality=90)
    print('иконка: %s (%d байт)' % (OUT_ICO, len(ico)))
    print('превью: %s' % OUT_PREVIEW)


if __name__ == '__main__':
    main()
