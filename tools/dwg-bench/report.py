"""Turns dwg-bench's results.jsonl into the comparison (spike).

    python3 report.py <work-dir> <corpus-dir> > report.md

Every library is measured on the same files with the same operations. Entity
types are normalized first: LibreDWG lists structural records (BLOCK,
ENDBLK, SEQEND, VERTEX_*, ATTRIB) as entities and names a few types
differently; the codecs do not.
"""

from __future__ import annotations

import json
import re
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

#: A job result or a summary, as read from results.jsonl.
Row = dict[str, Any]

LIBS = ["acadrust", "opencadcodec", "libredwg"]
STRUCTURAL = {
    "BLOCK",
    "ENDBLK",
    "SEQEND",
    "ATTRIB",
    "VERTEX_2D",
    "VERTEX_3D",
    "VERTEX_MESH",
    "VERTEX_PFACE",
    "VERTEX_PFACE_FACE",
}
RENAMED = {
    "DIMENSION_ANG2LN": "DIMENSION_ANGULAR_2LINE",
    "DIMENSION_ANG3PT": "DIMENSION_ANGULAR_3POINT",
    "POLYLINE_2D": "POLYLINE",
    "POLYLINE_3D": "POLYLINE",
    "POLYLINE_PFACE": "POLYLINE",
    "POLYLINE_MESH": "POLYLINE",
    "TABLE": "ACAD_TABLE",
}
DYNAMIC = re.compile(
    r"^BLOCK.*(PARAMETER|ACTION|GRIP|REPRESENTATION|PROPERTIESTABLE)"
    r"|PARAMETERENTITY$|GRIPENTITY$|EVALUATION_GRAPH|ENHANCEDBLOCK"
)
PARAMETRIC = re.compile(r"^ACDBASSOC|^ASSOC")


def types_of(summary: Row) -> Counter[str]:
    out: Counter[str] = Counter()
    for name, n in summary["types"].items():
        if name in STRUCTURAL:
            continue
        out[RENAMED.get(name, name)] += n
    return out


def objects_matching(summary: Row, rx: re.Pattern[str]) -> int:
    return int(sum(n for name, n in summary.get("objects", {}).items() if rx.search(name.upper())))


def same_fp(a: Row, b: Row) -> bool:
    if a["count"] != b["count"]:
        return False
    scale = max(1.0, abs(a["sum"]), abs(b["sum"]))
    return bool(abs(a["sum"] - b["sum"]) <= 1e-6 * scale)


def faithful(original: Row, again: Row) -> bool:
    """Same normalized entity types and counts, and the same geometry
    fingerprints."""
    return types_of(original) == types_of(again) and all(
        same_fp(original[k], again[k]) for k in ("lines", "circles", "inserts")
    )


def group_of(path: str, corpus: str) -> str:
    rel = path[len(corpus) :].lstrip("/") if path.startswith(corpus) else path
    return rel.split("/", 1)[0]


def outcome(r: Row) -> str:
    if r["ok"]:
        return "recovered" if (r.get("summary") or {}).get("recovered") else "ok"
    error = r.get("error") or ""
    if error.startswith("crashed"):
        return "crash"
    if error.startswith("timeout"):
        return "timeout"
    if error == "panicked":
        return "panic"
    return "error"


def pct(n: int, d: int) -> str:
    return f"{100 * n / d:.0f} % ({n}/{d})" if d else "—"


def main() -> None:
    work, corpus = Path(sys.argv[1]), str(Path(sys.argv[2]).resolve())
    rows = [json.loads(line) for line in (work / "results.jsonl").open()]
    out_dir = str((work / "out").resolve())
    originals = [r for r in rows if r["operation"] == "read" and not r["input"].startswith(out_dir)]
    reads = {(r["library"], r["input"]): r for r in rows if r["operation"] == "read"}
    writes = [r for r in rows if r["operation"] == "write"]
    files = sorted({r["input"] for r in originals})
    groups = sorted({group_of(f, corpus) for f in files})
    p = print

    p("# DWG/DXF library comparison: AcadRust, opencadcodec, LibreDWG\n")
    p(
        f"{len(files)} drawings, {len(rows)} operations, each in a process of its own "
        "(120 s limit).\n"
    )

    p("## 1. Reading the original drawings\n")
    p("| Group | Files | " + " | ".join(LIBS) + " |")
    p("|---|---|" + "---|" * len(LIBS))
    for g in [*groups, "all"]:
        in_group = [f for f in files if g == "all" or group_of(f, corpus) == g]
        cells = []
        for lib in LIBS:
            outs = Counter(outcome(reads[(lib, f)]) for f in in_group if (lib, f) in reads)
            good = outs["ok"] + outs["recovered"]
            extra = ", ".join(f"{k} {v}" for k, v in sorted(outs.items()) if k not in {"ok"})
            cells.append(pct(good, len(in_group)) + (f" [{extra}]" if extra else ""))
        p(f"| {g} | {len(in_group)} | " + " | ".join(cells) + " |")

    p("\n## 2. Speed and memory (files every library read)\n")
    common = [f for f in files if all(reads.get((lib, f), {}).get("ok") for lib in LIBS)]
    p(f"{len(common)} files.\n")
    p(
        "| Library | Read median ms | Read p95 ms | Read total s "
        "| Peak memory median MiB | Peak memory max MiB |"
    )
    p("|---|---|---|---|---|---|")
    for lib in LIBS:
        ms = sorted(reads[(lib, f)]["millis"] for f in common)
        mem = sorted((reads[(lib, f)].get("peak_kib") or 0) / 1024 for f in common)
        if ms:
            p95 = ms[min(len(ms) - 1, int(0.95 * len(ms)))]
            p(
                f"| {lib} | {statistics.median(ms):.1f} | {p95:.1f} | {sum(ms) / 1000:.1f} | "
                f"{statistics.median(mem):.0f} | {max(mem):.0f} |"
            )
    big = [f for f in files if "grande" in f]
    for f in big:
        p(
            f"\nLarge drawing `{Path(f).name}`: "
            + "; ".join(
                f"{lib} {outcome(reads[(lib, f)])} {reads[(lib, f)]['millis']:.0f} ms "
                f"{(reads[(lib, f)].get('peak_kib') or 0) / 1024:.0f} MiB"
                for lib in LIBS
                if (lib, f) in reads
            )
        )

    p("\n## 3. Agreement between libraries on the originals\n")
    p("Normalized entity types and counts, LINE/CIRCLE/INSERT fingerprints.\n")
    for a, b in [
        ("acadrust", "opencadcodec"),
        ("acadrust", "libredwg"),
        ("opencadcodec", "libredwg"),
    ]:
        both = [
            f for f in files if reads.get((a, f), {}).get("ok") and reads.get((b, f), {}).get("ok")
        ]
        agree = sum(faithful(reads[(a, f)]["summary"], reads[(b, f)]["summary"]) for f in both)
        p(f"- {a} vs {b}: {pct(agree, len(both))}")

    p("\n## 4. Writing\n")
    p("| Library | Output | Written | Failed |")
    p("|---|---|---|---|")
    for lib in LIBS:
        by_kind: dict[str, Counter[str]] = defaultdict(Counter)
        for w in writes:
            if w["library"] != lib or not reads.get((lib, w["input"]), {}).get("ok"):
                continue
            kind = (
                "DWG 2018"
                if ".r2018." in w["output"]
                else ("DWG" if w["output"].endswith(".dwg") else "DXF")
            )
            if lib == "libredwg" and kind == "DWG":
                kind = "DWG 2000"
            if lib != "libredwg" and kind == "DWG":
                kind = "DWG 2018"
            by_kind[kind]["ok" if w["ok"] else "failed"] += 1
        for kind, c in sorted(by_kind.items()):
            total = c["ok"] + c["failed"]
            p(f"| {lib} | {kind} | {pct(c['ok'], total)} | {c['failed']} |")

    p("\n## 5. Round trip: read → write → read again\n")
    p(
        "Faithful = the re-read drawing has the same normalized types, counts and "
        "fingerprints as the original read by the writer.\n"
    )
    p("| Writer | Output | Re-read by | Readable | Faithful |")
    p("|---|---|---|---|---|")
    for writer in LIBS:
        for kind in ["dwg", "dxf", "r2018.dwg"]:
            outputs = [
                w
                for w in writes
                if w["library"] == writer
                and w["ok"]
                and w["output"].endswith("." + kind)
                and (kind == "r2018.dwg" or ".r2018." not in w["output"])
            ]
            if not outputs:
                continue
            for reader in LIBS:
                readable = faithful_n = 0
                for w in outputs:
                    again = reads.get((reader, w["output"]))
                    if again and again["ok"]:
                        readable += 1
                        if faithful(reads[(writer, w["input"])]["summary"], again["summary"]):
                            faithful_n += 1
                p(
                    f"| {writer} | {kind.upper()} | {reader} | {pct(readable, len(outputs))} | "
                    f"{pct(faithful_n, len(outputs))} |"
                )

    p("\n## 6. Dynamic blocks and parametric (associative) objects\n")
    for label, rx in [
        ("dynamic-block objects", DYNAMIC),
        ("associative/constraint objects", PARAMETRIC),
    ]:
        with_them = [
            f
            for f in files
            if any(
                reads.get((lib, f), {}).get("ok")
                and objects_matching(reads[(lib, f)]["summary"], rx)
                for lib in LIBS
            )
        ]
        p(f"\n### {label}: {len(with_them)} drawings contain them\n")
        p(
            "| Library | Read in N drawings | Objects read "
            "| Kept after own DWG round trip | Kept after own DXF round trip |"
        )
        p("|---|---|---|---|---|")
        for lib in LIBS:
            seen = total = 0
            kept = {"dwg": [0, 0], "dxf": [0, 0]}
            for f in with_them:
                r = reads.get((lib, f))
                if not (r and r["ok"]):
                    continue
                n = objects_matching(r["summary"], rx)
                seen += n > 0
                total += n
                for w in writes:
                    if (
                        w["library"] == lib
                        and w["input"] == f
                        and w["ok"]
                        and ".r2018." not in w["output"]
                    ):
                        kind = "dwg" if w["output"].endswith(".dwg") else "dxf"
                        again = reads.get((lib, w["output"]))
                        if n:
                            kept[kind][1] += n
                            if again and again["ok"]:
                                kept[kind][0] += min(n, objects_matching(again["summary"], rx))
            p(
                f"| {lib} | {seen}/{len(with_them)} | {total} | "
                f"{pct(kept['dwg'][0], kept['dwg'][1])} | {pct(kept['dxf'][0], kept['dxf'][1])} |"
            )

    p("\n## 7. 3D solids (ACIS geometry)\n")
    with_solids = [
        f
        for f in files
        if any(
            reads.get((lib, f), {}).get("ok")
            and reads[(lib, f)]["summary"].get("solids", {}).get("count")
            for lib in LIBS
        )
    ]
    p(f"{len(with_solids)} drawings have 3DSOLID, REGION, BODY or surfaces.\n")
    p(
        "| Library | Solids read | With ACIS geometry | ACIS kB "
        "| Geometry kept, own DWG round trip "
        "| Own DXF round trip |"
    )
    p("|---|---|---|---|---|---|")
    for lib in LIBS:
        n = data = 0
        kb = 0.0
        kept = {"dwg": [0, 0], "dxf": [0, 0]}
        for f in with_solids:
            r = reads.get((lib, f))
            if not (r and r["ok"]):
                continue
            sm = r["summary"]
            n += sm.get("solids", {}).get("count", 0)
            data += sm.get("solids_with_data", 0)
            kb += sm.get("solids", {}).get("sum", 0) / 1024
            want = sm.get("solids_with_data", 0)
            for w in writes:
                if (
                    w["library"] == lib
                    and w["input"] == f
                    and w["ok"]
                    and want
                    and ".r2018." not in w["output"]
                ):
                    kind = "dwg" if w["output"].endswith(".dwg") else "dxf"
                    again = reads.get((lib, w["output"]))
                    kept[kind][1] += want
                    if again and again["ok"]:
                        kept[kind][0] += min(want, again["summary"].get("solids_with_data", 0))
        p(
            f"| {lib} | {n} | {data} | {kb:.0f} | {pct(kept['dwg'][0], kept['dwg'][1])} | "
            f"{pct(kept['dxf'][0], kept['dxf'][1])} |"
        )

    p("\n## 8. BIM-relevant data: extended data and objects a library does not know\n")
    p(
        "| Library | Entities with XDATA | XRECORD objects | Unknown/proxy objects "
        "| XDATA kept, own DWG round trip | Unknown kept, own DWG round trip |"
    )
    p("|---|---|---|---|---|---|")
    unknown = re.compile(r"UNKNOWN|PROXY|^AEC")

    def unknown_count(sm: Row) -> int:
        return objects_matching(sm, unknown) + int(
            sum(n for t, n in sm["types"].items() if unknown.search(t.upper()))
        )

    for lib in LIBS:
        xd = xr = unk = 0
        kept_x = [0, 0]
        kept_u = [0, 0]
        for f in files:
            r = reads.get((lib, f))
            if not (r and r["ok"]):
                continue
            sm = r["summary"]
            xd += sm.get("xdata_entities", 0)
            xr += sm.get("objects", {}).get("XRECORD", 0)
            u = unknown_count(sm)
            unk += u
            for w in writes:
                if (
                    w["library"] == lib
                    and w["input"] == f
                    and w["ok"]
                    and w["output"].endswith(".dwg")
                    and ".r2018." not in w["output"]
                ):
                    again = reads.get((lib, w["output"]))
                    again_sm = again["summary"] if again is not None and again["ok"] else None
                    if sm.get("xdata_entities", 0):
                        kept_x[1] += sm["xdata_entities"]
                        if again_sm is not None:
                            kept_x[0] += min(
                                sm["xdata_entities"], again_sm.get("xdata_entities", 0)
                            )
                    if u:
                        kept_u[1] += u
                        if again_sm is not None:
                            kept_u[0] += min(u, unknown_count(again_sm))
        p(
            f"| {lib} | {xd} | {xr} | {unk} | {pct(kept_x[0], kept_x[1])} | "
            f"{pct(kept_u[0], kept_u[1])} |"
        )

    p("\n## 9. Damaged drawings\n")
    damaged = [f for f in files if group_of(f, corpus) == "damaged"]
    p("| File | " + " | ".join(LIBS) + " |")
    p("|---|" + "---|" * len(LIBS))
    for f in damaged:
        p(
            f"| {Path(f).name} | "
            + " | ".join(outcome(reads[(lib, f)]) if (lib, f) in reads else "—" for lib in LIBS)
            + " |"
        )

    p("\n## 10. Stability over every operation\n")
    p("| Library | Operations | Crashes | Timeouts | Panics | Errors |")
    p("|---|---|---|---|---|---|")
    for lib in LIBS:
        mine = [r for r in rows if r["library"] == lib]
        c = Counter(outcome(r) for r in mine)
        p(f"| {lib} | {len(mine)} | {c['crash']} | {c['timeout']} | {c['panic']} | {c['error']} |")

    p("\n## 11. Kinds of data kept after saving and reopening\n")
    p(
        "Entities of each kind in the originals each library read, and the share "
        "still there when it re-reads its own DWG and DXF.\n"
    )
    kinds = {
        "Dimensions": re.compile(r"^DIMENSION|^ARC_DIMENSION"),
        "Hatches": re.compile(r"^HATCH$|^MPOLYGON$"),
        "Block references": re.compile(r"^INSERT$|^MINSERT$"),
        "Texts": re.compile(r"^TEXT$|^MTEXT$|^ATTDEF$"),
        "Leaders": re.compile(r"^LEADER$|^MULTILEADER$"),
        "Polylines": re.compile(r"^LWPOLYLINE$|^POLYLINE$"),
        "3D solids, regions, surfaces": re.compile(r"^3DSOLID$|^REGION$|^BODY$|SURFACE$"),
        "Meshes and 3D faces": re.compile(r"^MESH$|^3DFACE$|^POLYLINE_MESH$"),
        "Tables": re.compile(r"^ACAD_TABLE$"),
        "Images, underlays, wipeouts": re.compile(r"IMAGE|UNDERLAY|^WIPEOUT$"),
    }
    p("| Kind | " + " | ".join(f"{lib} read | DWG kept | DXF kept" for lib in LIBS) + " |")
    p("|---|" + "---|---|---|" * len(LIBS))
    for label, rx in kinds.items():
        cells = []
        for lib in LIBS:
            read_n = 0
            kept = {"dwg": [0, 0], "dxf": [0, 0]}
            for f in files:
                r = reads.get((lib, f))
                if not (r and r["ok"]):
                    continue
                n = sum(v for t, v in types_of(r["summary"]).items() if rx.search(t))
                read_n += n
                if not n:
                    continue
                for w in writes:
                    if (
                        w["library"] == lib
                        and w["input"] == f
                        and w["ok"]
                        and ".r2018." not in w["output"]
                    ):
                        kind = "dwg" if w["output"].endswith(".dwg") else "dxf"
                        again = reads.get((lib, w["output"]))
                        kept[kind][1] += n
                        if again is not None and again["ok"]:
                            m = sum(
                                v for t, v in types_of(again["summary"]).items() if rx.search(t)
                            )
                            kept[kind][0] += min(n, m)
            cells.append(
                f"{read_n} | {pct(kept['dwg'][0], kept['dwg'][1])} | "
                f"{pct(kept['dxf'][0], kept['dxf'][1])}"
            )
        p(f"| {label} | " + " | ".join(cells) + " |")

    p("\n## 12. AcadRust and opencadcodec, drawing by drawing\n")
    p(
        "opencadcodec is a fork of AcadRust; every drawing where the two behave "
        "differently: reading, what they read, or writing.\n"
    )
    p("| Drawing | Difference |")
    p("|---|---|")
    for f in files:
        up, fork = reads.get(("acadrust", f)), reads.get(("opencadcodec", f))
        name = f[len(corpus) :].lstrip("/") if f.startswith(corpus) else f
        notes = []
        if up is None or fork is None:
            continue
        if outcome(up) != outcome(fork):
            notes.append(f"read: AcadRust {outcome(up)}, opencadcodec {outcome(fork)}")
        if up["ok"] and fork["ok"]:
            ta, to = types_of(up["summary"]), types_of(fork["summary"])
            for t in sorted(set(ta) | set(to)):
                if ta[t] != to[t]:
                    notes.append(f"{t}: {ta[t]} vs {to[t]}")
            oa, oo = up["summary"].get("objects", {}), fork["summary"].get("objects", {})
            for t in sorted(set(oa) | set(oo)):
                if oa.get(t, 0) != oo.get(t, 0):
                    notes.append(f"{t}: {oa.get(t, 0)} vs {oo.get(t, 0)}")
        for w_a in writes:
            if w_a["library"] != "acadrust" or w_a["input"] != f:
                continue
            twin = w_a["output"].replace("/out/acadrust/", "/out/opencadcodec/")
            w_o = next(
                (w for w in writes if w["library"] == "opencadcodec" and w["output"] == twin), None
            )
            if w_o is not None and w_a["ok"] != w_o["ok"]:
                ext = w_a["output"].rsplit(".", 1)[-1].upper()
                err = (w_a.get("error") or w_o.get("error") or "")[:70]
                notes.append(
                    f"write {ext}: AcadRust {'ok' if w_a['ok'] else 'refused'}, "
                    f"opencadcodec {'ok' if w_o['ok'] else 'refused'} ({err})"
                )
        if notes:
            p(f"| {name} | " + "; ".join(notes[:6]) + (" …" if len(notes) > 6 else "") + " |")


if __name__ == "__main__":
    main()
