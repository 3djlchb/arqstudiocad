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


class QtWorker(QObject):
    """Runs tasks on worker threads and calls back on the GUI thread."""

    _finished = Signal(object, object)  # (done callback, Outcome)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._pool = ThreadPoolExecutor(max_workers=WORKERS, thread_name_prefix="arq-work")
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
