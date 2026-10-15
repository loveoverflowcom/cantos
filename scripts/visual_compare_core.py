"""Deterministic pixel accounting and spatial grouping; no image/runtime dependency."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    width: int
    height: int

    def __post_init__(self):
        values = (self.x, self.y, self.width, self.height)
        if any(type(value) is not int for value in values):
            raise ValueError("rectangle coordinates must be integers")
        if self.x < 0 or self.y < 0 or self.width <= 0 or self.height <= 0:
            raise ValueError("rectangle must have a nonnegative origin and positive size")

    def require_inside(self, width, height):
        if self.x + self.width > width or self.y + self.height > height:
            raise ValueError("rectangle extends beyond image dimensions")


def analyze_mask(width, height, changed, masks=(), tile_size=16):
    """Count every unmasked change; group adjacent occupied tiles, never discard noise.

    Coordinates are physical pixels, with exclusive right/bottom edges. Bounding boxes
    retain the exact changed-pixel bounds inside each group. Tile adjacency merges nearby
    changes; it is a spatial aid, not identification of a DOM or Compose component.
    """
    if type(width) is not int or type(height) is not int or min(width, height) <= 0:
        raise ValueError("positive integer image dimensions required")
    if type(tile_size) is not int or not 1 <= tile_size <= 128:
        raise ValueError("tile size must be an integer from 1 to 128")
    total = width * height
    if total > 16_000_000 or len(changed) != total:
        raise ValueError("mask length/dimensions invalid or above 16 million pixels")
    if ((width + tile_size - 1) // tile_size) * ((height + tile_size - 1) // tile_size) > 262_144:
        raise ValueError("tile grid exceeds 262144 cells; increase tile size")
    if any(value not in (0, 255) for value in changed):
        raise ValueError("mask must contain only 0 or 255")
    excluded = bytearray(total)
    for rect in masks:
        rect.require_inside(width, height)
        for y in range(rect.y, rect.y + rect.height):
            start = y * width + rect.x
            excluded[start:start + rect.width] = b"\xff" * rect.width
    excluded_pixels = excluded.count(255)
    if excluded_pixels == total:
        raise ValueError("masks exclude every pixel; comparison would be vacuous")
    visible = bytes(0 if excluded[i] else value for i, value in enumerate(changed))
    tiles = {}
    for index, value in enumerate(visible):
        if not value:
            continue
        y, x = divmod(index, width)
        key = (x // tile_size, y // tile_size)
        bounds = tiles.setdefault(key, [x, y, x, y, 0])
        bounds[0] = min(bounds[0], x)
        bounds[1] = min(bounds[1], y)
        bounds[2] = max(bounds[2], x)
        bounds[3] = max(bounds[3], y)
        bounds[4] += 1
    regions = []
    while tiles:
        first = next(iter(tiles))
        stack = [first]
        left, top, right, bottom, pixels = tiles.pop(first)
        while stack:
            tx, ty = stack.pop()
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    key = (tx + dx, ty + dy)
                    bounds = tiles.pop(key, None)
                    if bounds is not None:
                        left, top = min(left, bounds[0]), min(top, bounds[1])
                        right, bottom = max(right, bounds[2]), max(bottom, bounds[3])
                        pixels += bounds[4]
                        stack.append(key)
        rw, rh = right - left + 1, bottom - top + 1
        regions.append(dict(x=left, y=top, width=rw, height=rh, pixels=pixels,
                            local_ratio=pixels / (rw * rh),
                            severity_hint="review-large-area" if rw * rh >= total * .05
                            else "review-small-area"))
    regions.sort(key=lambda r: (-r["pixels"], r["y"], r["x"]))
    for number, region in enumerate(regions, 1):
        region["id"] = number
    pixels = visible.count(255)
    return dict(total_pixels=total, compared_pixels=total - excluded_pixels,
                excluded_pixels=excluded_pixels, diff_pixels=pixels,
                diff_ratio=pixels / total,
                unmasked_diff_ratio=pixels / (total - excluded_pixels),
                regions=regions), visible
