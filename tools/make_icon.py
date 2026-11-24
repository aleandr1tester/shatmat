# -*- coding: utf-8 -*-
"""Иконка ShatMat: белая шашка (круг с ободком) и чёрный ферзь внутри неё.

Скрипт самодостаточный (только Pillow): разбирает полигоны из
pieces/black_queen.svg, рисует иконку в нескольких размерах и собирает
assets/icon.ico. В exe иконку вшивает build.rs через windres.
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

# Палитра шашки: тёплый ободок и кремовый диск.
RIM_OUTER = (176, 138, 92, 255)
RIM_MID = (198, 160, 112, 255)
DISC_LIGHT = (247, 240, 226, 255)
DISC_EDGE = (230, 219, 197, 255)
QUEEN_DARK = (30, 28, 40, 255)
QUEEN_INK = (10, 10, 16, 255)


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


POLYS = svg_polygons(SVG_QUEEN)


def radial_fill(size, inner, outer):
    """Круг с мягким радиальным градиентом от inner к outer."""
    grad = Image.radial_gradient('L').resize((size, size), Image.BICUBIC)
    base = Image.new('RGBA', (size, size), inner)
    edge = Image.new('RGBA', (size, size), outer)
    base.paste(edge, (0, 0), grad)
    return base


def render(size, ss=4):
    """Рисует иконку в размере size (с суперсэмплингом ss)."""
    S = size * ss
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cy = S / 2
    rad = S * 0.485  # шашка почти во весь холст

    # Ободок шашки: внешнее тёмное кольцо и светлый скос.
    d.ellipse([cx - rad, cy - rad, cx + rad, cy + rad], fill=RIM_OUTER)
    rim_in = rad * 0.945
    d.ellipse([cx - rim_in, cy - rim_in, cx + rim_in, cy + rim_in],
              fill=RIM_MID)
    # световой скос на ободке (слева сверху)
    d.arc([cx - rim_in, cy - rim_in, cx + rim_in, cy + rim_in],
          start=200, end=320, fill=(232, 200, 158, 255),
          width=max(1, int(S * 0.012)))

    # Кремовый диск с градиентом к краю.
    disc = rad * 0.845
    disc_img = radial_fill(S, DISC_LIGHT, DISC_EDGE)
    mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(mask).ellipse(
        [cx - disc, cy - disc, cx + disc, cy + disc], fill=255)
    img.paste(disc_img, (0, 0), mask)
    d = ImageDraw.Draw(img)
    d.ellipse([cx - disc, cy - disc, cx + disc, cy + disc],
              outline=(214, 201, 176, 255), width=max(1, int(S * 0.006)))

    # Чёрный ферзь внутри шашки.
    x0, y0, x1, y1 = 4.0, 9.0, 96.0, 95.0
    qh = disc * 1.72
    scale = qh / (y1 - y0)
    qw = (x1 - x0) * scale
    ox = cx - qw / 2 - x0 * scale
    oy = cy + disc * 0.90 - y1 * scale

    def to_pts(poly):
        return [(ox + px * scale, oy + py * scale) for px, py in poly]

    for poly_list, fill, stroke, sw in POLYS:
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
    return head + entries + data


def montage(images):
    """Превью: размеры на тёмном и светлом фоне."""
    sizes = [s for s, _ in images if s in (16, 24, 32, 48, 64, 128)]
    W, H = 1240, 420
    img = Image.new('RGB', (W, H), (24, 22, 32))
    d = ImageDraw.Draw(img)
    d.rectangle([0, 210, W, H], fill=(242, 240, 246))
    by_size = dict(images)
    x = 40
    for s in sizes:
        im = by_size[s]
        img.paste(im, (x, 105 - s // 2), im)
        img.paste(im, (x, 315 - s // 2), im)
        d.text((x, 175), '%d px' % s, fill=(150, 145, 165))
        d.text((x, 385), '%d px' % s, fill=(110, 105, 125))
        x += max(s, 40) + 34
    big = by_size[256]
    img.paste(big, (W - 300, 82), big)
    d.text((W - 300, 175), '256 px', fill=(150, 145, 165))
    return img


def main():
    os.makedirs(ASSETS, exist_ok=True)
    images = [(s, render(s)) for s in SIZES]
    ico = build_ico(images)
    io.open(OUT_ICO, 'wb').write(ico)
    images[-1][1].save(OUT_PNG)
    montage(images).save(OUT_PREVIEW, 'JPEG', quality=92)
    print('иконка: %s (%d байт)' % (OUT_ICO, len(ico)))
    print('превью: %s' % OUT_PREVIEW)


if __name__ == '__main__':
    main()
