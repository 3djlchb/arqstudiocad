"""Shared by the tests that check the GIL is released (ADR 0012 D4.6, ADR 0013 D5.3)."""

import threading
import time
from collections.abc import Callable
from pathlib import Path


def big_dxf(path: Path, lines: int) -> Path:
    """An R12 DXF with many LINE entities: slow enough to open and save."""
    codes = ["0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009", "0", "ENDSEC"]
    codes += ["0", "SECTION", "2", "ENTITIES"]
    for i in range(lines):
        codes += ["0", "LINE", "8", "0", "10", f"{i}.0", "20", "0.0", "30", "0.0"]
        codes += ["11", f"{i}.5", "21", "1.0", "31", "0.0"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")
    return path


def share_of_time_other_threads_ran(call: Callable[[], object]) -> tuple[float, float]:
    """How much a counting thread advanced during `call`, relative to its pace
    when the main thread just sleeps: near 1 if `call` released the GIL, near 0
    if it held it (then only a switch interval or two slips through). Also
    returns how long `call` took."""
    ticks = 0
    done = threading.Event()

    def count() -> None:
        nonlocal ticks
        while not done.is_set():
            ticks += 1

    counter = threading.Thread(target=count)
    counter.start()
    try:
        start, t0 = ticks, time.perf_counter()
        time.sleep(0.2)
        pace = (ticks - start) / (time.perf_counter() - t0)
        start, t0 = ticks, time.perf_counter()
        call()
        elapsed = time.perf_counter() - t0
        ran = ticks - start
    finally:
        done.set()
        counter.join()
    return ran / (pace * elapsed), elapsed
