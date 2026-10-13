#!/usr/bin/env bash
# A report from the floor machine (ADR 0007, ADR 0013 D5.8): hardware and
# driver details, build, tests, self-check and timings, in one text file to
# send back.
#
# Run it on the real machine (LMDE 7 with the GT 730), from the repository
# root, after the README's "Building from source" steps for Linux:
#   bash tools/reference_machine_report.sh
# Optional: ARQ_CORPUS_DIR=/path/to/drawings also checks those drawings
# (their file names then appear in the report).
#
# Every section runs even if an earlier one fails; the summary at the end
# lists what passed. Nothing is installed or sent anywhere.
set -uo pipefail

REPORT="target/reference-report-$(date +%Y%m%d-%H%M).txt"
mkdir -p target
exec > >(tee "$REPORT") 2>&1
declare -a RESULTS=()

section() { printf '\n==================== %s ====================\n' "$1"; }
record() { RESULTS+=("$1: $2"); }
run() { # run NAME COMMAND... : runs it timed, records PASS/FAIL
    local name=$1; shift
    local start=$SECONDS
    if "$@"; then record "$name" "PASS ($((SECONDS - start)) s)"
    else record "$name" "FAIL (exit $?, $((SECONDS - start)) s)"; fi
}
have() { command -v "$1" > /dev/null 2>&1; }

section "System"
date -Is
grep -E '^(PRETTY_NAME|VERSION)=' /etc/os-release
uname -srm
echo "session: ${XDG_SESSION_TYPE:-unknown}  desktop: ${XDG_CURRENT_DESKTOP:-unknown}"
echo "cores: $(nproc)"
free -h | head -2
grep -m1 'model name' /proc/cpuinfo
for flag in sse4_2 popcnt avx avx2 fma avx512f; do
    grep -qw "$flag" /proc/cpuinfo && echo "  cpu has $flag" || echo "  cpu lacks $flag"
done

section "Graphics"
have lspci && lspci -nnk | grep -A3 -Ei 'vga|3d controller'
if have glxinfo; then glxinfo -B; else echo "glxinfo missing: sudo apt install mesa-utils"; fi
if have vulkaninfo; then
    # The instance version, then every device with its type (hardware or CPU).
    vulkaninfo --summary 2>&1 | grep -m1 'Vulkan Instance Version'
    vulkaninfo --summary 2>&1 | sed -n '/^Devices:/,$p'
else echo "vulkaninfo missing: sudo apt install vulkan-tools"; fi

section "Toolchain"
git log -1 --format='commit %h %s (%cd)' --date=short
echo "branch: $(git branch --show-current)"
have cargo && cargo --version
have uv && uv --version

section "Build"
run "build (uv sync)" uv sync

section "Rust tests"
run "rust tests" cargo test

section "Python tests"
run "python tests" env QT_QPA_PLATFORM=offscreen uv run pytest -q -p no:cacheprovider

section "Installation self-check"
run "self-check" uv run arqstudiocad --check

section "Timings"
run "timings" uv run python - <<'EOF'
"""Open, query and save a generated drawing, with timings."""
import os
import tempfile
import time
from pathlib import Path

from arqstudiocad.core import CadApplication, CadBounds2

def dxf(path: Path, lines: int) -> None:
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for i in range(lines):
        x, y = float(i % 500), float(i // 500)
        codes += ["0", "LINE", "8", "0", "10", str(x), "20", str(y), "30", "0.0"]
        codes += ["11", str(x + 0.5), "21", str(y + 0.5), "31", "0.0"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")

def timed(label: str, call):
    t0 = time.perf_counter()
    value = call()
    print(f"  {label:<34} {1000 * (time.perf_counter() - t0):9.1f} ms")
    return value

app = CadApplication()
with tempfile.TemporaryDirectory() as tmp:
    for lines in (10_000, 100_000):
        print(f"{lines} lines:")
        source = Path(tmp) / f"lines-{lines}.dxf"
        dxf(source, lines)
        doc = timed("open DXF", lambda: app.open_document(source))
        timed("get_entities", doc.get_entities)
        timed("get_bounds", doc.get_bounds)
        timed("query_entities (small window)",
              lambda: doc.query_entities(CadBounds2((10, 10), (20, 20))))
        ids = [e.id for e in doc.get_entities()[:1000]]
        timed("move 1000 entities", lambda: doc.move_entities(ids, (1, 0)))
        timed("undo", doc.undo)
        timed("save DWG", lambda: doc.save(Path(tmp) / "out.dwg"))
        timed("save DXF", lambda: doc.save(Path(tmp) / "out.dxf"))
        doc.close()
print(f"cpu count seen by Python: {os.cpu_count()}")
EOF

if [[ -n "${ARQ_CORPUS_DIR:-}" ]]; then
    section "Real drawings"
    run "corpus" cargo test -p arq-core --test corpus -- --ignored local --nocapture
fi

section "Summary"
printf '%s\n' "${RESULTS[@]}"
echo
echo "Report saved to $REPORT — send that file back."
