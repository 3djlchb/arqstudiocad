"""Background work for commands, with Qt (the commands' ``Worker``).

Work runs on a thread pool; its outcome comes back through a Qt signal, so
the command resumes on the GUI thread, never on the worker.
"""

from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from typing import Any

from PySide6.QtCore import QObject, Signal

from arqstudiocad.commands import Outcome

#: Opening and saving are rarely more than one at a time; the core already
#: uses every core inside each of them.
WORKERS = 2

#: The graphics checks at start (the renderer's probe, then the cards'
#: survey) run one after the other on a thread of their own, so opening and
#: saving keep both of theirs: sharing them cost a drawing opened at start
#: 290 ms on the reference machine (2026-11-11).
GRAPHICS_WORKERS = 1


class QtWorker(QObject):
    """Runs tasks on worker threads and calls back on the GUI thread."""

    _finished = Signal(object, object)  # (done callback, Outcome)

    def __init__(
        self, parent: QObject | None = None, workers: int = WORKERS, name: str = "arq-work"
    ) -> None:
        super().__init__(parent)
        #: How many tasks run at once.
        self.workers = workers
        self._pool = ThreadPoolExecutor(max_workers=workers, thread_name_prefix=name)
        self._finished.connect(self._deliver)
        self.destroyed.connect(lambda: self._pool.shutdown(wait=False))

    def run(self, task: Callable[[], Any], done: Callable[[Outcome], None]) -> None:
        def job() -> None:
            try:
                outcome = Outcome(value=task())
            except Exception as error:
                outcome = Outcome(error=error)
            self._finished.emit(done, outcome)  # queued to the GUI thread

        self._pool.submit(job)

    def shutdown(self) -> None:
        """Waits for running work; for quitting."""
        self._pool.shutdown(wait=True)

    def _deliver(self, done: Callable[[Outcome], None], outcome: Outcome) -> None:
        done(outcome)
