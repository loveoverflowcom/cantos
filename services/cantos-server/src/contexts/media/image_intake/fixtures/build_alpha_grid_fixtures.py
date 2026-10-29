#!/usr/bin/env python3
"""Builds the synthetic AVIF alpha and ImageGrid fixtures of the Media admission tests.

Pillow (libavif) encodes each tile and the single-item alpha image. Pillow cannot write
ImageGrid items, so this script assembles the ISOBMFF container by hand from those encoded
tiles (ISO/IEC 23008-12 grid + MIAF), exactly as libavif lays out `avifenc --grid`. The
assembled files are then decoded back with Pillow's libavif and compared with the source
pixels, which is the independent oracle: this script never decodes AV1 itself.

usage: build_alpha_grid_fixtures.py OUT_DIR
"""
import struct
import sys
from pathlib import Path

from PIL import Image

ALPHA_URN = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\x00"


def box(kind, payload):
    return struct.pack(">I4s", 8 + len(payload), kind) + payload


def full_box(kind, version, flags, payload):
    return box(kind, struct.pack(">I", (version << 24) | flags) + payload)


def children(data, start=0, end=None):
    end = len(data) if end is None else end
    while start < end:
        size, kind = struct.unpack(">I4s", data[start : start + 8])
        yield kind, start + 8, start + size
        start += size


def read_tile(data):
    """Return (ftyp box, hdlr box, {item: payload}, properties) of one Pillow AVIF."""
    ftyp = hdlr = None
    meta = None
    for kind, begin, end in children(data):
        if kind == b"ftyp":
            ftyp = data[begin - 8 : end]
        if kind == b"meta":
            meta = (begin + 4, end)
    items, locations, props, assoc = [], {}, [], {}
    for kind, begin, end in children(data, *meta):
        if kind == b"hdlr":
            hdlr = data[begin - 8 : end]
        elif kind == b"iinf":
            for _, b2, e2 in children(data, begin + 6, end):
                items.append(struct.unpack(">H", data[b2 + 4 : b2 + 6])[0])
        elif kind == b"iloc":
            version = data[begin]
            offset_size, length_size = data[begin + 4] >> 4, data[begin + 4] & 15
            base_size = data[begin + 5] >> 4
            assert version in (0, 1) and base_size == 0
            count = struct.unpack(">H", data[begin + 6 : begin + 8])[0]
            cursor = begin + 8
            for _ in range(count):
                item = struct.unpack(">H", data[cursor : cursor + 2])[0]
                cursor += 2 + (2 if version else 0) + 2  # id, [method], data ref
                assert struct.unpack(">H", data[cursor : cursor + 2])[0] == 1
                cursor += 2
                offset = int.from_bytes(data[cursor : cursor + offset_size], "big")
                cursor += offset_size
                length = int.from_bytes(data[cursor : cursor + length_size], "big")
                cursor += length_size
                locations[item] = data[offset : offset + length]
        elif kind == b"iprp":
            for k2, b2, e2 in children(data, begin, end):
                if k2 == b"ipco":
                    props = [(k3, data[b3 - 8 : e3]) for k3, b3, e3 in children(data, b2, e2)]
                elif k2 == b"ipma":
                    count = struct.unpack(">I", data[b2 + 4 : b2 + 8])[0]
                    cursor = b2 + 8
                    for _ in range(count):
                        item = struct.unpack(">H", data[cursor : cursor + 2])[0]
                        n = data[cursor + 2]
                        assoc[item] = [i & 0x7F for i in data[cursor + 3 : cursor + 3 + n]]
                        cursor += 3 + n
    return ftyp, hdlr, locations, props, assoc


def encode_tile(image, subsampling):
    path = Path(sys.argv[1]) / "_tile.avif"
    image.save(path, quality=100, subsampling=subsampling, speed=6)
    return path.read_bytes()


def tile_parts(data):
    """Split one Pillow RGBA tile into color/alpha payloads and their property boxes."""
    ftyp, hdlr, locations, props, assoc = read_tile(data)
    named = {i: (kind, raw) for i, (kind, raw) in enumerate(props, 1)}

    def pick(item, wanted):
        return [named[i][1] for i in assoc[item] if named[i][0] in wanted]

    return {
        "ftyp": ftyp,
        "hdlr": hdlr,
        "color": locations[1],
        "alpha": locations.get(2),
        "color_props": pick(1, {b"ispe", b"pixi", b"av1C"}),
        "colr": pick(1, {b"colr"}),
        "alpha_props": pick(2, {b"ispe", b"pixi", b"av1C"}) if 2 in assoc else None,
    }


def infe(item, kind, hidden):
    return full_box(b"infe", 2, 1 if hidden else 0, struct.pack(">HH4s", item, 0, kind) + b"\x00")


def assemble_grid(tiles, rows, columns, out_w, out_h, with_alpha):
    """Color ImageGrid (+ alpha ImageGrid) over the tiles' `av01` payloads."""
    count = rows * columns
    color_ids = list(range(1, count + 1))
    alpha_ids = list(range(count + 1, 2 * count + 1)) if with_alpha else []
    color_grid = 2 * count + 1 if with_alpha else count + 1
    alpha_grid = color_grid + 1
    descriptor = struct.pack(">BBBBHH", 0, 0, rows - 1, columns - 1, out_w, out_h)
    payloads = [(i, t["color"]) for i, t in zip(color_ids, tiles)]
    payloads += [(i, t["alpha"]) for i, t in zip(alpha_ids, tiles)]
    payloads.append((color_grid, descriptor))
    if with_alpha:
        payloads.append((alpha_grid, descriptor))

    first = tiles[0]
    # ipco (1-based): 1 tile ispe, 2 tile pixi, 3 tile av1C, 4 colr, 5 grid ispe,
    # then for alpha: 6 alpha pixi, 7 alpha av1C, 8 auxC
    ipco = list(first["color_props"]) + list(first["colr"])
    ipco.append(full_box(b"ispe", 0, 0, struct.pack(">II", out_w, out_h)))
    assoc = {i: [1, 2, 3] for i in color_ids}
    assoc[color_grid] = [5, 4]
    if with_alpha:
        alpha_pixi, alpha_av1c = first["alpha_props"][1], first["alpha_props"][2]
        ipco += [alpha_pixi, alpha_av1c, full_box(b"auxC", 0, 0, ALPHA_URN)]
        for i in alpha_ids:
            assoc[i] = [1, 6, 7]
        assoc[alpha_grid] = [5, 8]

    items = [infe(i, b"av01", True) for i in color_ids + alpha_ids]
    items.append(infe(color_grid, b"grid", False))
    if with_alpha:
        items.append(infe(alpha_grid, b"grid", True))
    iinf = full_box(b"iinf", 0, 0, struct.pack(">H", len(items)) + b"".join(items))

    refs = box(b"dimg", struct.pack(">HH", color_grid, count) + b"".join(struct.pack(">H", i) for i in color_ids))
    if with_alpha:
        refs += box(b"dimg", struct.pack(">HH", alpha_grid, count) + b"".join(struct.pack(">H", i) for i in alpha_ids))
        refs += box(b"auxl", struct.pack(">HHH", alpha_grid, 1, color_grid))
    iref = full_box(b"iref", 0, 0, refs)

    ipma_entries = b"".join(
        struct.pack(">HB", item, len(idx)) + bytes(idx) for item, idx in sorted(assoc.items())
    )
    iprp = box(
        b"iprp",
        box(b"ipco", b"".join(ipco)) + full_box(b"ipma", 0, 0, struct.pack(">I", len(assoc)) + ipma_entries),
    )
    pitm = full_box(b"pitm", 0, 0, struct.pack(">H", color_grid))

    def build(mdat_offset):
        locs, offset = [], mdat_offset
        for item, payload in payloads:
            locs.append(struct.pack(">HHHII", item, 0, 1, offset, len(payload)))
            offset += len(payload)
        iloc = full_box(b"iloc", 0, 0, bytes([0x44, 0x00]) + struct.pack(">H", len(locs)) + b"".join(locs))
        meta = full_box(b"meta", 0, 0, first["hdlr"] + pitm + iloc + iinf + iref + iprp)
        return first["ftyp"] + meta

    head = build(0)
    mdat_payload_offset = len(head) + 8
    head = build(mdat_payload_offset)
    return head + box(b"mdat", b"".join(p for _, p in payloads))


def source_image(width, height):
    image = Image.new("RGBA", (width, height))
    pixels = image.load()
    for y in range(height):
        for x in range(width):
            pixels[x, y] = (
                (x * 255) // (width - 1),
                (y * 255) // (height - 1),
                ((x // 8 + y // 8) % 2) * 180 + 40,
                max(0, 255 - ((x - width // 2) ** 2 + (y - height // 2) ** 2) // 8) if x > 10 else 0,
            )
    return image


def build_grid(source, tile, with_alpha, subsampling):
    width, height = source.size
    columns, rows = -(-width // tile), -(-height // tile)
    padded = Image.new("RGBA", (columns * tile, rows * tile))
    padded.paste(source, (0, 0))
    # Surplus edge pixels repeat the border so tile codec noise cannot hide a layout error.
    for x in range(width, columns * tile):
        padded.paste(source.crop((width - 1, 0, width, height)), (x, 0))
    for y in range(height, rows * tile):
        padded.paste(padded.crop((0, height - 1, columns * tile, height)), (0, y))
    tiles = []
    for row in range(rows):
        for column in range(columns):
            crop = padded.crop((column * tile, row * tile, (column + 1) * tile, (row + 1) * tile))
            tiles.append(tile_parts(encode_tile(crop, subsampling)))
    return assemble_grid(tiles, rows, columns, width, height, with_alpha)


def main(out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    single = source_image(64, 48)
    single.save(out / "pillow-alpha-64x48.avif", quality=100, subsampling="4:4:4", speed=6)
    big = source_image(120, 100)
    (out / "grid-2x2-120x100.avif").write_bytes(build_grid(big.convert("RGB").convert("RGBA"), 64, False, "4:4:4"))
    (out / "grid-alpha-2x2-120x100.avif").write_bytes(build_grid(big, 64, True, "4:4:4"))
    (out / "_tile.avif").unlink()


if __name__ == "__main__":
    main(sys.argv[1])
