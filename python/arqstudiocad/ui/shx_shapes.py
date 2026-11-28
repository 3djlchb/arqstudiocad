"""Read the line-vector subset of AutoCAD shape-library files for previews."""

import os
import struct
from functools import cache
from math import atan, atan2, cos, pi, sin
from pathlib import Path

from arqstudiocad.paths import linetypes_dir

_SHX_SEARCH_PATHS: list[Path] = []


def register_shx_search_path(path: Path) -> None:
    """Search the directory of a loaded LIN file for its referenced SHX files."""
    directory = path.expanduser().resolve()
    if directory.is_dir():
        _SHX_SEARCH_PATHS[:] = [item for item in _SHX_SEARCH_PATHS if item != directory]
        _SHX_SEARCH_PATHS.insert(0, directory)


def find_shx_file(name: str) -> Path | None:
    """Find an SHX file beside a loaded LIN file or in configured font folders."""
    candidate = Path(name).expanduser()
    folders = [*_SHX_SEARCH_PATHS, linetypes_dir(), Path.home() / ".local/share/arqstudiocad/fonts"]
    folders.extend(
        Path(path).expanduser()
        for path in os.environ.get("ARQSTUDIOCAD_SHX_PATH", "").split(os.pathsep)
        if path
    )
    candidates = (
        [candidate] if candidate.is_absolute() else [folder / candidate for folder in folders]
    )
    return next((path for path in candidates if path.is_file()), None)


ShapeStrokes = tuple[int, str, tuple[tuple[tuple[float, float], ...], ...]]


@cache
def _shape_records(path: str, modified_ns: int) -> dict[str, ShapeStrokes]:
    del modified_ns
    data = Path(path).read_bytes()
    marker = data.find(b"\x1a")
    if marker < 0 or not data[:marker].lower().startswith(b"autocad-86 shapes 1."):
        return {}
    offset = marker + 1
    if len(data) < offset + 6:
        return {}
    count = struct.unpack_from("<H", data, offset + 4)[0]
    offset += 6
    index: list[tuple[int, int]] = []
    for _ in range(count):
        if offset + 4 > len(data):
            return {}
        shape_number, length = struct.unpack_from("<HH", data, offset)
        index.append((shape_number, length))
        offset += 4
    result = {}
    for shape_number, length in index:
        body = data[offset : offset + length]
        offset += length
        if len(body) != length:
            break
        nul = body.find(b"\0")
        if nul > 0 and all(32 <= byte < 127 for byte in body[:nul]):
            name = body[:nul].decode("ascii").casefold()
            body = body[nul + 1 :]
        else:
            name = f"#{shape_number}".casefold()
        paths = _decode_vectors(body)
        shape = (shape_number, name, paths)
        result[name] = shape
        result[f"#{shape_number}".casefold()] = shape
    return result


def shape_strokes(filename: str, shape_name: str) -> ShapeStrokes | None:
    """Returns a shape number and its decoded line paths, if supported."""
    path = find_shx_file(filename)
    if path is None:
        return None
    try:
        records = _shape_records(str(path), path.stat().st_mtime_ns)
    except OSError:
        return None
    return records.get(shape_name.casefold())


def _decode_vectors(data: bytes) -> tuple[tuple[tuple[float, float], ...], ...]:
    x = y = 0.0
    scale = 1.0
    pen_down = True
    saved: list[tuple[float, float, float, bool]] = []
    path: list[tuple[float, float]] = [(x, y)]
    paths: list[tuple[tuple[float, float], ...]] = []

    def move(dx: float, dy: float) -> None:
        nonlocal x, y, path
        if pen_down:
            path.extend(((x + dx * scale, y + dy * scale),))
        else:
            if len(path) > 1:
                paths.append(tuple(path))
            path = [(x + dx * scale, y + dy * scale)]
        x += dx * scale
        y += dy * scale

    def draw_arc(center: tuple[float, float], radius: float, start: float, sweep: float) -> None:
        nonlocal x, y, path
        steps = max(4, int(abs(sweep) * max(radius, 1.0) / 0.5))
        points = [
            (
                center[0] + radius * cos(start + sweep * step / steps),
                center[1] + radius * sin(start + sweep * step / steps),
            )
            for step in range(steps + 1)
        ]
        if pen_down:
            if not path:
                path.append(points[0])
            path.extend(points[1:])
        else:
            if len(path) > 1:
                paths.append(tuple(path))
            path = [points[-1]]
        x, y = points[-1]

    def octant_arc(
        radius: float, signed_spec: int, start_offset: float = 0, end_offset: float = 0
    ) -> None:
        direction = -1 if signed_spec < 0 else 1
        spec = abs(signed_spec)
        start_octant, span = (spec >> 4) & 7, spec & 7
        span = 8 if span == 0 else span
        start_angle = (start_octant + direction * start_offset) * pi / 4
        end_angle = (start_octant + direction * span + direction * end_offset) * pi / 4
        sweep = end_angle - start_angle
        scaled_radius = radius * scale
        center = (x - scaled_radius * cos(start_angle), y - scaled_radius * sin(start_angle))
        draw_arc(center, scaled_radius, start_angle, sweep)

    def bulge_arc(dx: int, dy: int, bulge: int) -> None:
        if not bulge:
            move(dx, dy)
            return
        vx, vy = dx * scale, dy * scale
        chord = (vx * vx + vy * vy) ** 0.5
        sweep = 4 * atan(bulge / 127)
        if not chord or not sweep:
            move(dx, dy)
            return
        radius = chord / (2 * sin(abs(sweep) / 2))
        midpoint = (x + vx / 2, y + vy / 2)
        offset = radius * cos(abs(sweep) / 2)
        direction = 1 if sweep > 0 else -1
        center = (
            midpoint[0] - direction * vy / chord * offset,
            midpoint[1] + direction * vx / chord * offset,
        )
        start = atan2(y - center[1], x - center[0])
        draw_arc(center, radius, start, sweep)

    cursor = 0
    while cursor < len(data):
        opcode = data[cursor]
        cursor += 1
        if opcode == 0:
            break
        if opcode == 1:
            pen_down = True
            path = [(x, y)]
        elif opcode == 2:
            pen_down = False
            if len(path) > 1:
                paths.append(tuple(path))
            path = [(x, y)]
        elif opcode in (3, 4) and cursor < len(data):
            factor = data[cursor]
            cursor += 1
            if factor:
                scale = scale / factor if opcode == 3 else scale * factor
        elif opcode == 5:
            saved.append((x, y, scale, pen_down))
        elif opcode == 6 and saved:
            x, y, scale, pen_down = saved.pop()
            path = [(x, y)]
        elif opcode == 8 and cursor + 2 <= len(data):
            dx, dy = struct.unpack_from("bb", data, cursor)
            cursor += 2
            move(dx, dy)
        elif opcode == 9:
            while cursor + 2 <= len(data):
                dx, dy = struct.unpack_from("bb", data, cursor)
                cursor += 2
                if dx == dy == 0:
                    break
                move(dx, dy)
        elif opcode == 10 and cursor + 2 <= len(data):
            radius, signed_spec = data[cursor], struct.unpack_from("b", data, cursor + 1)[0]
            cursor += 2
            octant_arc(float(radius), signed_spec)
        elif opcode == 11 and cursor + 5 <= len(data):
            start_offset, end_offset, high_radius, radius, spec = data[cursor : cursor + 5]
            cursor += 5
            signed_spec = struct.unpack("b", bytes((spec,)))[0]
            octant_arc(
                float((high_radius << 8) | radius),
                signed_spec,
                start_offset / 256,
                end_offset / 256,
            )
        elif opcode == 12 and cursor + 3 <= len(data):
            dx, dy, bulge = struct.unpack_from("bbb", data, cursor)
            cursor += 3
            bulge_arc(dx, dy, bulge)
        elif opcode == 13:
            while cursor + 2 <= len(data):
                dx, dy = struct.unpack_from("bb", data, cursor)
                cursor += 2
                if dx == dy == 0 or cursor >= len(data):
                    break
                bulge = struct.unpack_from("b", data, cursor)[0]
                cursor += 1
                if bulge:
                    bulge_arc(dx, dy, bulge)
                else:
                    move(dx, dy)
        elif opcode >= 0x10:
            length, direction = opcode >> 4, opcode & 0x0F
            angle = direction * pi / 8.0
            move(length * cos(angle), length * sin(angle))
        else:
            # Subshape opcodes are left out of this small preview reader.
            break
    if len(path) > 1:
        paths.append(tuple(path))
    return tuple(paths)
