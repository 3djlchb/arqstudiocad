"""Compare complete native observations from progecad_table_corpus.lsp.

This complements compatibility_matrix.py: a core-verified save does not prove
native table fidelity. Raw entity metadata is retained as evidence but excluded
from comparison because runtime entity references change between sessions.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from typing import Any

TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"|[()]|[^\s()]+')


def lisp_value(text: str) -> Any:
    """Read the scalar/dotted-list subset emitted for cell properties."""
    tokens = TOKEN.findall(text)
    offset = 0

    def read() -> Any:
        nonlocal offset
        if offset >= len(tokens):
            raise ValueError("truncated native value")
        token = tokens[offset]
        offset += 1
        if token == "(":
            values: list[Any] = []
            while offset < len(tokens) and tokens[offset] != ")":
                if tokens[offset] == ".":
                    offset += 1
                    if len(values) != 1:
                        raise ValueError("invalid dotted property")
                    values.append(read())
                    if offset >= len(tokens) or tokens[offset] != ")":
                        raise ValueError("invalid dotted property tail")
                    break
                values.append(read())
            if offset >= len(tokens):
                raise ValueError("unclosed native value")
            offset += 1
            return values
        if token in (")", "."):
            raise ValueError("unexpected native delimiter")
        if token.startswith('"'):
            return json.loads(token)
        try:
            value = float(token)
        except ValueError as error:
            raise ValueError(f"unsupported native value: {token}") from error
        if not math.isfinite(value):
            raise ValueError("nonfinite native measurement")
        return value

    result = read()
    if offset != len(tokens):
        raise ValueError("trailing native value")
    return result


def observation(path: Path, expected_drawing: str, tables: int) -> dict[str, Any]:
    """Reject duplicate, empty and incomplete captures, including missing cells."""
    result: dict[str, Any] = {}
    seen: set[str] = set()
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        key, separator, value = line.partition("\t")
        if not separator or key in seen:
            raise ValueError(f"missing separator or duplicate key: {key}")
        seen.add(key)
        if key.endswith("/entity_dxf"):
            continue
        result[key] = lisp_value(value)
    if result.get("schema") != "progecad-table-v1":
        raise ValueError("unknown or missing native observation schema")
    if result.get("drawing") != expected_drawing:
        raise ValueError("observation belongs to a different drawing")
    prefixes = {"/".join(key.split("/")[:2]) for key in result if key.startswith("table/")}
    if tables <= 0 or len(prefixes) != tables:
        raise ValueError(f"expected {tables} tables, observed {len(prefixes)}")
    for prefix in prefixes:
        rows, columns = (result.get(f"{prefix}/{name}") for name in ("rows", "columns"))
        if not (
            isinstance(rows, float)
            and isinstance(columns, float)
            and rows.is_integer()
            and columns.is_integer()
            and 0 < rows <= 10000
            and 0 < columns <= 10000
        ):
            raise ValueError("invalid native table dimensions")
        rows, columns = int(rows), int(columns)
        required = {
            f"{prefix}/{field}"
            for field in ("height", "horizontal_margin", "vertical_margin", "style")
        }
        required.update(f"{prefix}/text_height/{kind}" for kind in (1, 2, 4))
        required.update(f"{prefix}/column_width/{col}" for col in range(columns))
        required.update(f"{prefix}/row_height/{row}" for row in range(rows))
        for row in range(rows):
            for col in range(columns):
                required.update(f"{prefix}/{field}/{row}/{col}" for field in ("text", "cell_state"))
                key = f"{prefix}/cell_dxf/{row * columns + col}"
                required.add(key)
                cell = result.get(key)
                if not isinstance(cell, list) or not all(
                    isinstance(p, list) and len(p) == 2 for p in cell
                ):
                    raise ValueError(f"invalid or missing cell record: {key}")
                if not {171, 173, 175, 176} <= {pair[0] for pair in cell}:
                    raise ValueError(f"incomplete merge metadata: {key}")
        missing = required - result.keys()
        if missing:
            raise ValueError(f"missing native measurements: {sorted(missing)}")
        for key in required:
            if not any(field in key for field in ("/text/", "/cell_dxf/", "/style")):
                value = result[key]
                if not isinstance(value, float) or value < 0:
                    raise ValueError(f"invalid numeric measurement: {key}")
    return result


def equivalent(a: Any, b: Any) -> bool:
    if isinstance(a, float) and isinstance(b, float):
        return math.isclose(a, b, rel_tol=0.0, abs_tol=1e-8)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(equivalent(x, y) for x, y in zip(a, b, strict=True))
    return type(a) is type(b) and a == b


def compare(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    keys = (before.keys() | after.keys()) - {"drawing"}
    differences = [
        {"key": key, "before": before.get(key), "after": after.get(key)}
        for key in sorted(keys)
        if key not in before or key not in after or not equivalent(before[key], after[key])
    ]
    return {"properties": len(keys), "matches": not differences, "differences": differences}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--baseline-drawing", required=True)
    parser.add_argument("--candidate-drawing", required=True)
    parser.add_argument("--tables", type=int, default=4)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        before = observation(args.baseline, args.baseline_drawing, args.tables)
        after = observation(args.candidate, args.candidate_drawing, args.tables)
        report = compare(before, after)
        report.update(
            {
                str(name): hashlib.sha256(path.read_bytes()).hexdigest()
                for name, path in (
                    ("baseline_sha256", args.baseline),
                    ("candidate_sha256", args.candidate),
                )
            }
        )
        if args.output.resolve() in {args.baseline.resolve(), args.candidate.resolve()} or (
            args.output.exists()
            and any(args.output.samefile(p) for p in (args.baseline, args.candidate))
        ):
            raise ValueError("report must not overwrite an observation")
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        return 0 if report["matches"] else 1
    except (ValueError, OSError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
