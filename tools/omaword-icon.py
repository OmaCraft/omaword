#!/usr/bin/env python3
"""Render the original OmaCraft icon family without third-party art or dependencies."""
from pathlib import Path
import struct
import zlib
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'assets/omaword-icon'
APP_ID = 'io.github.ustaaa.OmaWord'
ACCENT = '#5b8fe0'
VARIANT = 'word'
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)


PALETTES = {
    'omagrid': ('#173E34', '#287657', '#85D9A5'),
    'omaword': ('#192F52', '#315C9B', '#9EC8F9'),
    'omadeck': ('#4D2536', '#923F58', '#F2A5A8'),
    'omanotes': ('#36234F', '#72469E', '#CEA9F4'),
    'omacraft': ('#242B3A', '#3A4559', '#C5D1DF'),
}

def scene(name="omaword"):
    dark, mid, light = PALETTES[name]
    paper = '#FAF6EB'
    s = [('roundrect', (8,8,240,240,52), dark),
         ('roundrect', (16,16,224,224,45), mid),
         ('roundrect', (24,24,208,208,38), dark)]
    if name == 'omacraft':
        for x,y,color in [(54,54,'#85D9A5'),(132,54,'#9EC8F9'),(54,132,'#F2A5A8'),(132,132,'#CEA9F4')]:
            s += [('roundrect',(x,y,70,70,19),color)]
        s += [('polygon', ([(104,104),(152,104),(152,152),(104,152)],), dark)]
        return s
    # A single folded document, shared silhouette. Open upper corner is our family mark.
    s += [('roundrect',(59,48,144,168,18),'#101722'),
          ('polygon', ([(52,42),(151,42),(194,85),(194,202),(52,202)],),paper),
          ('polygon', ([(151,42),(151,85),(194,85)],),light)]
    if name == 'omagrid':
        for row in range(3):
            for col in range(3):
                s += [('roundrect',(71+35*col,103+28*row,28,21,4),mid if row==0 else light)]
        s += [('roundrect',(106,131,28,21,4),dark)]
    elif name == 'omaword':
        s += [('roundrect',(72,102,45,12,4),mid)]
        for y,w in [(128,103),(150,103),(172,70)]:
            s += [('roundrect',(72,y,w,8,4),mid)]
    elif name == 'omadeck':
        s += [('roundrect',(69,103,108,71,7),mid),
              ('rect',(82,142,17,19),paper),('rect',(107,128,17,33),light),
              ('rect',(132,115,17,46),paper),('roundrect',(102,183,42,6,3),mid)]
    else:
        s += [('rect',(52,42,13,160),mid),('roundrect',(78,105,24,24,5),mid),
              ('roundrect',(113,112,60,8,4),mid),('roundrect',(78,147,95,8,4),mid),
              ('roundrect',(78,169,65,8,4),mid),('roundrect',(185,104,16,31,5),light),
              ('roundrect',(185,143,16,31,5),mid)]
    return s


def inside(shape, x, y):
    kind, data, _ = shape
    if kind == 'rect':
        a, b, w, h = data
        return a <= x < a+w and b <= y < b+h
    if kind == 'roundrect':
        a, b, w, h, r = data
        if not (a <= x < a+w and b <= y < b+h):
            return False
        qx = max(a+r-x, 0, x-(a+w-r))
        qy = max(b+r-y, 0, y-(b+h-r))
        return qx*qx + qy*qy <= r*r
    if kind == 'polygon':
        (points,) = data
        hit = False
        j = len(points)-1
        for i, (xi, yi) in enumerate(points):
            xj, yj = points[j]
            if (yi > y) != (yj > y) and x < (xj-xi)*(y-yi)/(yj-yi)+xi:
                hit = not hit
            j = i
        return hit
    return False


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


def png(size, shapes):
    # 2x2 coverage sampling keeps even the 16 px output crisp and deterministic.
    rows = bytearray()
    steps = 2
    for py in range(size):
        rows.append(0)
        for px in range(size):
            sums = [0, 0, 0, 0]
            for sy in range(steps):
                for sx in range(steps):
                    x = (px + (sx + .5)/steps) * 256 / size
                    y = (py + (sy + .5)/steps) * 256 / size
                    color = (0, 0, 0, 0)
                    for shape in shapes:
                        if inside(shape, x, y):
                            h = shape[2].lstrip('#')
                            color = tuple(int(h[i:i+2], 16) for i in (0, 2, 4)) + (255,)
                    for i, value in enumerate(color):
                        sums[i] += value
            rows.extend((value + steps*steps//2)//(steps*steps) for value in sums)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(bytes(rows), 9)) + chunk(b'IEND', b''))


def svg(shapes):
    elements = []
    for kind, data, color in shapes:
        if kind == 'rect':
            x, y, w, h = data
            elements.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}"/>')
        elif kind == 'roundrect':
            x, y, w, h, r = data
            elements.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{color}"/>')
        else:
            (pts,) = data
            points = ' '.join(f'{x},{y}' for x,y in pts)
            elements.append(f'<polygon points="{points}" fill="{color}"/>')
    return '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">\n' + '\n'.join(elements) + '\n</svg>\n'


shapes = scene()
DEST.mkdir(parents=True, exist_ok=True)
rendered = {size: png(size, shapes) for size in SIZES}
for size in SIZES:
    folder = DEST / str(size)
    folder.mkdir(parents=True, exist_ok=True)
    (folder / f'{APP_ID}.png').write_bytes(rendered[size])
vector = svg(shapes)
(DEST / f'{APP_ID}.svg').write_text(vector, encoding='utf-8')
# The icon container stores all standard sizes, each as a PNG-compressed ICO frame.
frames = [(s, rendered[s]) for s in (16, 24, 32, 48, 64, 128, 256)]
header = struct.pack('<HHH', 0, 1, len(frames))
offset = 6 + 16*len(frames)
entries = bytearray()
for size, data in frames:
    entries += struct.pack('<BBBBHHII', 0 if size == 256 else size, 0 if size == 256 else size, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
(DEST / 'omaword.ico').write_bytes(header + entries + b''.join(data for _, data in frames))
