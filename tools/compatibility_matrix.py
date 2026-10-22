"""Measure the integrated core's DWG/DXF compatibility (milestone 03.4).

Run from the project environment, for example:
    uv run python tools/compatibility_matrix.py target/corpus --output target/matrix.md

Each drawing runs in a child process with a timeout. Sources are only read;
the core saves copies in a temporary directory and never accepts data losses.
This measures clean reading and core-verified saving, not visual fidelity,
external application compatibility, editing or parameter recomputation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Literal

from arqstudiocad import __version__
from arqstudiocad.core import (
    CadApplication,
    CadDocument,
    CadError,
    LossyConversionError,
)

FORMATS = ("dwg", "dxf")
ReadStatus = Literal["ok", "recovered", "error", "timeout", "crash"]
SaveStatus = Literal["verified", "blocked_loss", "error", "not_run"]


@dataclass
class RoundTrip:
    format: str
    status: SaveStatus = "not_run"
    version: str | None = None
    losses: list[str] = field(default_factory=list)
    error: str | None = None


@dataclass
class Drawing:
    source: str
    read: ReadStatus = "error"
    sha256: str | None = None
    version: str | None = None
    entities: int = 0
    model_types: dict[str, int] = field(default_factory=dict)
    dynamic_definitions: int = 0
    undecoded_objects: list[str] = field(default_factory=list)
    recovery: list[str] = field(default_factory=list)
    error: str | None = None
    roundtrips: list[RoundTrip] = field(default_factory=lambda: [RoundTrip(ext) for ext in FORMATS])


def drawings(inputs: list[Path]) -> list[Path]:
    """Find drawings, deduplicating overlapping roots; an empty scan fails."""
    found: set[Path] = set()
    for source in inputs:
        if not source.exists():
            raise ValueError(f"input does not exist: {source}")
        candidates = source.rglob("*") if source.is_dir() else [source]
        for path in candidates:
            if path.is_file() and path.suffix.lower().lstrip(".") in FORMATS:
                found.add(path.resolve())
    if not found:
        raise ValueError("the corpus is empty: no DWG/DXF drawing was checked")
    return sorted(found)


def roundtrip(app: CadApplication, document: CadDocument, target: Path) -> RoundTrip:
    """A verified save or an explicit refusal; never consent to a loss."""
    result = RoundTrip(target.suffix.lstrip("."))
    try:
        document.save(target)  # Rust owns safe writing and read-back validation.
        back = app.open_document(target)
        try:
            result.version = back.version
            if back.recovery_problems:
                result.status = "error"
                result.error = "saved copy reopened by recovery: " + "; ".join(
                    back.recovery_problems
                )
            else:
                result.status = "verified"
        finally:
            back.close()
    except LossyConversionError as error:
        result.status = "blocked_loss"
        result.losses = error.losses
        result.error = f"{error.code}: {error}"
    except (CadError, OSError) as error:
        result.status = "error"
        result.error = str(error)
    return result


def measure(source: Path) -> Drawing:
    """Inspect one input with the public Python API, without modifying it."""
    result = Drawing(str(source))
    app = CadApplication()
    try:
        result.sha256 = hashlib.sha256(source.read_bytes()).hexdigest()
        doc = app.open_document(source)
        result.version = doc.version
        result.entities = doc.entity_count
        result.model_types = dict(sorted(Counter(e.kind for e in doc.get_entities()).items()))
        result.dynamic_definitions = sum(doc.is_dynamic_block(name) for name in doc.block_names)
        result.undecoded_objects = doc.undecoded_objects
        result.recovery = doc.recovery_problems
        result.read = "recovered" if result.recovery else "ok"
        if result.read == "ok":
            with tempfile.TemporaryDirectory(prefix="arq-matrix-") as folder:
                result.roundtrips = [
                    roundtrip(app, doc, Path(folder) / f"copy.{ext}") for ext in FORMATS
                ]
    except (CadError, OSError) as error:
        result.read = "error"
        result.error = str(error)
    finally:
        for document in app.documents():
            document.close()
    return result


def isolated(source: Path, timeout: float) -> Drawing:
    """A crash/hang is a failed row, never a missing or successful row."""
    try:
        completed = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--worker", str(source)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return Drawing(str(source), read="timeout", error=f"timeout after {timeout:g} seconds")
    except OSError as error:
        return Drawing(str(source), error=str(error))
    if completed.returncode:
        return Drawing(
            str(source), read="crash", error=f"worker exited with code {completed.returncode}"
        )
    try:
        # The codec may print diagnostics before the worker's final JSON line.
        data = json.loads(completed.stdout.splitlines()[-1])
        trips = [RoundTrip(**row) for row in data.pop("roundtrips")]
        result = Drawing(**data, roundtrips=trips)
        if result.source != str(source) or [r.format for r in trips] != list(FORMATS):
            raise ValueError("worker returned a different input or incomplete formats")
        return result
    except (ValueError, TypeError, KeyError, IndexError, AttributeError) as error:
        return Drawing(str(source), error=f"invalid worker result: {error}")


def passed(rows: list[Drawing]) -> bool:
    return bool(rows) and all(
        row.read == "ok"
        and [trip.format for trip in row.roundtrips] == list(FORMATS)
        and all(trip.status == "verified" for trip in row.roundtrips)
        for row in rows
    )


def cell(text: str) -> str:
    """Keep paths and diagnostics inside their Markdown table cell."""
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace("|", "&#124;")
        .replace("\r", " ")
        .replace("\n", " ")
    )


def provenance() -> dict[str, str]:
    """Identify the measurement environment and checked-out dependency lock."""
    script = Path(__file__).resolve()
    return {
        "core_version": __version__,
        "generated_at": datetime.now(UTC).isoformat(),
        "platform": platform.platform(),
        "python": platform.python_version(),
        "cargo_lock_sha256": hashlib.sha256(
            (script.parents[1] / "Cargo.lock").read_bytes()
        ).hexdigest(),
        "tool_sha256": hashlib.sha256(script.read_bytes()).hexdigest(),
    }


def markdown(rows: list[Drawing]) -> str:
    lines = [
        "# DWG/DXF compatibility measurements",
        "",
        f"Core {__version__}; {len(rows)} drawing(s).",
        "",
        *[f"- {key}: {value}" for key, value in provenance().items()],
        "",
        "Counts describe model-space entity kinds. A verified save passed the core's",
        "safe-save checks and reopened without recovery. Losses are never accepted.",
        "These results do not establish visual fidelity, Xref resolution, BIM semantics,",
        "editing, recomputation or independent-application compatibility.",
        "",
        "## By source format, version and model-space kind",
        "",
        "A drawing with several kinds contributes once to each kind; rows are not additive.",
        "Save columns count whole drawings, not individually verified entities.",
        "",
        "| Source format | Version | Kind | Files | Clean reads | Recovered "
        "| DWG verified | DXF verified |",
        "|---|---|---|---|---|---|---|---|",
    ]
    groups: dict[tuple[str, str, str], list[Drawing]] = defaultdict(list)
    for row in rows:
        for kind in row.model_types or {"(no observed model-space entities)": 0}:
            source_format = Path(row.source).suffix.lstrip(".").upper()
            groups[(source_format, row.version or "unknown", kind)].append(row)
    for (source_format, version, kind), group in sorted(groups.items()):
        counts = [sum(row.read == status for row in group) for status in ("ok", "recovered")]
        saves = [
            sum(
                any(t.format == ext and t.status == "verified" for t in row.roundtrips)
                for row in group
            )
            for ext in FORMATS
        ]
        lines.append(
            f"| {cell(source_format)} | {cell(version)} | {cell(kind)} | {len(group)} "
            f"| {counts[0]} | {counts[1]} "
            f"| {saves[0]} | {saves[1]} |"
        )
    lines += [
        "",
        "## Per drawing",
        "",
        "| Source | Version | Read | DWG | DXF | Diagnostics |",
        "|---|---|---|---|---|---|",
    ]
    for row in rows:
        diagnostics = [*row.recovery, *row.undecoded_objects]
        if row.error:
            diagnostics.append(row.error)
        for trip in row.roundtrips:
            if trip.error:
                diagnostics.append(f"{trip.format}: {trip.error}")
        lines.append(
            f"| {cell(row.source)} | {cell(row.version or 'unknown')} | {row.read} | "
            + " | ".join(
                trip.status + (f" / {trip.version}" if trip.version else "")
                for trip in row.roundtrips
            )
            + f" | {cell('; '.join(diagnostics))} |"
        )
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if len(args) == 2 and args[0] == "--worker":
        print(json.dumps(asdict(measure(Path(args[1]))), ensure_ascii=True))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "inputs", nargs="+", type=Path, help="drawing files or recursive corpus roots"
    )
    parser.add_argument("--format", choices=("markdown", "json"), default="markdown")
    parser.add_argument("--output", type=Path, help="report path; defaults to standard output")
    parser.add_argument(
        "--timeout", type=float, default=120, help="seconds per drawing (default 120)"
    )
    options = parser.parse_args(args)
    try:
        if not math.isfinite(options.timeout) or options.timeout <= 0:
            raise ValueError("timeout must be a positive finite number")
        sources = drawings(options.inputs)
        if options.output is not None:
            aliases_input = options.output.resolve() in sources or (
                options.output.exists()
                and any(options.output.samefile(source) for source in sources)
            )
            if aliases_input:
                raise ValueError("the output report must not overwrite an input drawing")
        rows = []
        for index, source in enumerate(sources, start=1):
            print(f"[{index}/{len(sources)}] {source}", file=sys.stderr)
            rows.append(isolated(source, options.timeout))
        if options.format == "json":
            report = (
                json.dumps(
                    {
                        "schema_version": 1,
                        **provenance(),
                        "scope": (
                            "clean read and core-verified save; entity kinds in model space only"
                        ),
                        "drawings": [asdict(row) for row in rows],
                    },
                    indent=2,
                    ensure_ascii=True,
                )
                + "\n"
            )
        else:
            report = markdown(rows)
        if options.output is None:
            print(report, end="")
        else:
            options.output.parent.mkdir(parents=True, exist_ok=True)
            options.output.write_text(report, encoding="utf-8")
        return 0 if passed(rows) else 1
    except (OSError, ValueError) as error:
        print(f"compatibility matrix: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
