"""Checks the GPU viewport on this machine (ADR 0034 D6.4) and writes a report.

Run it from the repository root on the machine to try, after the README's
"Building from source" steps:

    uv run python tools/gpu_viewport_check.py

It probes every graphics backend of the platform, each in a child process
(ADR 0007). Then, for each one that passes on a hardware adapter and for the
CPU, it opens a real viewport in a window of its own and:

- times panning and zooming a generated drawing (``--entities``, 100,000 by
  default);
- draws a small scene with every overlay (selection, highlight, grips, a
  window, a dropped block's note, the crosshair) and saves its pixels as
  shown on screen;
- compares each GPU's pixels with the CPU's. A blank capture, all but
  one colour, is reported, not compared: nothing was drawn, or nothing
  could be read back.

Windows appear and close on their own; leave the mouse still. The report
goes to ``target/gpu-viewport-report-<date>.txt``; send that file back. The
images it compares are kept beside it.
"""

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from collections import Counter
from collections.abc import Callable
from datetime import datetime
from pathlib import Path
from typing import Any

#: How long a child may take before it is given up, in seconds.
CHILD_TIMEOUT = 300

#: A capture this much of one colour is blank. The overlay scene's
#: background covers about 84% of it; a black window on Windows 11, all but
#: its rounded corners.
BLANK = 0.99


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--entities", type=int, default=100_000)
    parser.add_argument("--child", help=argparse.SUPPRESS)
    parser.add_argument("--out", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.child is not None:
        return child(args.child, args.entities, Path(args.out))
    return report(args.entities)


def report(entities: int) -> int:
    from arqstudiocad import __version__
    from arqstudiocad.gpu import backend_order, probe_backend

    folder = Path("target")
    folder.mkdir(exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d-%H%M")
    images = folder / f"gpu-viewport-{stamp}"
    images.mkdir(exist_ok=True)
    lines: list[str] = []

    def say(text: str = "") -> None:
        print(text, flush=True)
        lines.append(text)

    say(f"GPU viewport check, {datetime.now().isoformat(timespec='seconds')}")
    say(f"version {__version__}; {platform.platform()}; Python {platform.python_version()}")
    say(f"processor {platform.processor() or platform.machine()}; cores {os.cpu_count()}")
    say()
    say("== Probes (each in a child process)")
    passed: list[str] = []
    for backend in backend_order():
        result = probe_backend(backend)
        if result.available:
            kind = "software" if result.software else "hardware"
            say(f"  {backend:9} {kind:8} {result.adapter} ({result.device_type})")
            if not result.software and backend != "software":
                passed.append(backend)
        else:
            say(f"  {backend:9} failed   {result.error}")
    say()

    runs: dict[str, dict[str, Any]] = {}
    for name in [*passed, "cpu"]:
        say(f"== Viewport: {name}")
        env = dict(os.environ)
        env.pop("ARQ_GPU_VIEWPORT", None)
        if name != "cpu":
            env["ARQ_GPU_VIEWPORT"] = name
        with tempfile.TemporaryDirectory() as tmp:
            done = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    name,
                    "--entities",
                    str(entities),
                    "--out",
                    str((images / f"{name}.png").resolve()),
                ],
                capture_output=True,
                text=True,
                timeout=CHILD_TIMEOUT,
                env=env,
                cwd=tmp,
                check=False,
            )
        if done.returncode != 0 or not done.stdout.strip():
            say(f"  failed (exit {done.returncode}): {done.stderr.strip()[-600:]}")
            say()
            continue
        result = json.loads(done.stdout.strip().splitlines()[-1])
        runs[name] = result
        say(f"  drawn by: {result['drawn_by']}")
        say(f"  pan  median {result['pan'][0]} ms, p95 {result['pan'][1]} ms")
        say(f"  zoom median {result['zoom'][0]} ms, p95 {result['zoom'][1]} ms")
        say()

    if "cpu" in runs:
        say("== Overlays, GPU against CPU (pixels the other did not draw within one)")
        for name in passed:
            if name in runs:
                try:
                    misses = compare(images / f"{name}.png", images / "cpu.png")
                except ValueError as error:
                    say(f"  {name:9} not compared: {error}")
                    continue
                say(f"  {name:9} {misses} pixel(s)" + ("" if misses else ": the same"))
        say()
    say(f"Images: {images}")
    path = folder / f"gpu-viewport-report-{stamp}.txt"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"\nReport saved to {path}: send that file back.")
    return 0


def child(name: str, entities: int, out: Path) -> int:
    """One viewport, drawn by the GPU on ``name`` or by the CPU."""
    from PySide6.QtCore import QPointF, Qt
    from PySide6.QtWidgets import QApplication

    from arqstudiocad.core import CadApplication, CadDocument
    from arqstudiocad.ui.viewport import Viewport

    app = QApplication([])
    cad = CadApplication()

    def window(document: CadDocument) -> Viewport:
        viewport = Viewport(document)
        viewport.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint)
        viewport.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        viewport.setGeometry(100, 100, 1200, 650)
        viewport.show()
        viewport.raise_()
        settle(app, 1.0)
        return viewport

    # Timing: a generated drawing of lines, circles and arcs.
    source = Path("drawing.dxf")
    source.write_text(drawing(entities), encoding="ascii")
    big = cad.open_document(source)
    viewport = window(big)
    drawn_by = viewport.gpu
    if name != "cpu" and drawn_by is None:
        print("the GPU did not attach; see the log above", file=sys.stderr)
        return 1

    def frames(action: Callable[[int], None], count: int = 60) -> tuple[float, float]:
        times: list[float] = []
        for i in range(count):
            action(i)
            viewport._redraw()
            start = time.perf_counter()
            viewport.repaint()
            times.append((time.perf_counter() - start) * 1e3)
        times = times[3:]
        p95 = sorted(times)[int(len(times) * 0.95) - 1]
        return round(statistics.median(times), 1), round(p95, 1)

    pan = frames(lambda i: viewport.view.pan(7.0 if i % 20 < 10 else -7.0, 3.0))
    zoom = frames(lambda i: viewport.view.zoom_at(600, 325, 1.03 if i % 30 < 15 else 1 / 1.03))
    viewport.deleteLater()
    settle(app, 0.2)
    big.close()

    # Overlays: a small scene with all of them, at a working zoom.
    document = cad.create_document("Overlays")
    first, _ = document.add_line((0.0, 0.0), (100.0, 60.0))
    second, _ = document.add_circle((60.0, 20.0), 15.0)
    document.add_line((0.0, 60.0), (100.0, 0.0))
    viewport = window(document)
    viewport.resize(400, 300)
    settle(app, 0.5)
    viewport.show_selection((first, second))
    grips = tuple(document.grips([first, second]))
    viewport.show_grips(grips, grips[0])
    viewport._set_hover(second)
    viewport._press, viewport._band = QPointF(40, 200), QPointF(260, 260)
    viewport._drop_name = "Door"
    viewport._cursor_at = QPointF(300.0, 120.0)
    image, previous = None, None
    for _ in range(60):
        viewport.repaint()
        settle(app, 0.1)
        image = capture(viewport)
        if image == previous:
            break
        previous = image
    if image is not None:
        image.save(str(out))
    viewport.deleteLater()
    settle(app, 0.2)
    print(
        json.dumps(
            {
                "drawn_by": list(drawn_by) if drawn_by else "the CPU",
                "pan": pan,
                "zoom": zoom,
            }
        )
    )
    return 0


def capture(viewport: Any) -> Any:
    """The viewport's pixels as shown on screen.

    On Windows, reading back a window that DirectX 12 or OpenGL presents to
    gives black: there the desktop, as composed, is read where the window
    lies (ADR 0034 D6.4). Elsewhere the window's own pixels are read, which
    a busy desktop's compositor cannot be redrawing at that moment.
    """
    from PySide6.QtCore import QPoint

    screen = viewport.screen()
    if sys.platform == "win32":
        at = viewport.mapToGlobal(QPoint(0, 0)) - screen.geometry().topLeft()
        shot = screen.grabWindow(0, at.x(), at.y(), viewport.width(), viewport.height())
    else:
        shot = screen.grabWindow(int(viewport.winId()))
    return shot.toImage()


def settle(app: object, seconds: float) -> None:
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        app.processEvents()  # type: ignore[attr-defined]
        time.sleep(0.02)


def drawing(entities: int) -> str:
    """A DXF of ``entities`` lines, circles and arcs on a grid."""
    side = int(entities**0.5) + 1
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for i in range(entities):
        x, y = (i % side) * 10.0, (i // side) * 10.0
        if i % 3 == 0:
            codes += ["0", "LINE", "8", "0", "10", str(x), "20", str(y), "30", "0"]
            codes += ["11", str(x + 6), "21", str(y + 3), "31", "0"]
        elif i % 3 == 1:
            codes += ["0", "CIRCLE", "8", "0", "10", str(x + 3), "20", str(y + 3), "30", "0"]
            codes += ["40", "2.5"]
        else:
            codes += ["0", "ARC", "8", "0", "10", str(x + 3), "20", str(y + 3), "30", "0"]
            codes += ["40", "2.5", "50", "17", "51", "160"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    return "\n".join(codes)


def compare(gpu_png: Path, cpu_png: Path) -> int:
    """Pixels one image draws clearly where the other draws nothing within
    one pixel, both ways, as the tests compare them.

    The background is the CPU image's commonest colour, as captured. Raises
    ``ValueError`` when the images cannot be compared: missing, of different
    sizes, or either blank (``BLANK``; it would otherwise pass: a black
    pixel is close enough to a dark background to count as drawing).
    """
    from PySide6.QtGui import QGuiApplication, QImage

    _ = QGuiApplication.instance() or QGuiApplication([])
    gpu, cpu = QImage(str(gpu_png)), QImage(str(cpu_png))
    if gpu.isNull() or cpu.isNull():
        raise ValueError("an image is missing")
    if gpu.size() != cpu.size():
        raise ValueError("the images differ in size")
    counts: dict[str, Counter[int]] = {}
    for label, image in (("GPU", gpu), ("CPU", cpu)):
        counts[label] = Counter(
            image.pixel(x, y) for y in range(image.height()) for x in range(image.width())
        )
        share = counts[label].most_common(1)[0][1] / (image.width() * image.height())
        if share >= BLANK:
            raise ValueError(f"the {label} image is blank, {share:.2%} of one colour")
    common = counts["CPU"].most_common(1)[0][0]
    r, g, b = (common >> 16) & 0xFF, (common >> 8) & 0xFF, common & 0xFF

    def contrast(image: QImage, x: int, y: int) -> int:
        c = image.pixelColor(x, y)
        return max(abs(c.red() - r), abs(c.green() - g), abs(c.blue() - b))

    def near(image: QImage, x: int, y: int) -> bool:
        return any(
            contrast(image, x + dx, y + dy) > 0
            for dx in (-1, 0, 1)
            for dy in (-1, 0, 1)
            if 0 <= x + dx < image.width() and 0 <= y + dy < image.height()
        )

    misses = 0
    for y in range(cpu.height()):
        for x in range(cpu.width()):
            for one, other in ((cpu, gpu), (gpu, cpu)):
                if contrast(one, x, y) > 48 and not near(other, x, y):
                    misses += 1
    return misses


if __name__ == "__main__":
    sys.exit(main())
