"""Turns failed tests into GitHub annotations (CI only).

GitHub Actions keeps a job's log behind authentication, but the annotations
of a check run are public. Printing one ``::error`` line per failed test makes
the failing test's name and message readable from the pull request and from
the public API, so an intermittent failure can be found without the log.

Usage, after ``pytest --junitxml=<file>`` failed:
``python tools/ci_test_annotations.py <file>``.
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

#: GitHub shows at most ten error annotations per step.
LIMIT = 10


@dataclass(frozen=True)
class Failure:
    test: str
    message: str


def failures(junit_xml: Path) -> list[Failure]:
    """Every failed or erroring test case in a JUnit XML report."""
    found = []
    for case in ET.parse(junit_xml).getroot().iter("testcase"):
        for kind in ("failure", "error"):
            problem = case.find(kind)
            if problem is None:
                continue
            name = f"{case.get('classname', '')}::{case.get('name', '')}".strip(":")
            text = problem.get("message") or (problem.text or "").strip()
            first = next((line for line in text.splitlines() if line.strip()), kind)
            found.append(Failure(name, first.strip()))
    return found


def escape_data(text: str) -> str:
    """Escapes a workflow command's message."""
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def escape_property(text: str) -> str:
    """Escapes a workflow command's property (the title)."""
    return escape_data(text).replace(":", "%3A").replace(",", "%2C")


def workflow_commands(found: list[Failure]) -> list[str]:
    """The ``::error`` lines for ``found``, at most ``LIMIT`` of them."""
    lines = [
        f"::error title={escape_property(f.test)}::{escape_data(f.message[:500])}"
        for f in found[:LIMIT]
    ]
    if len(found) > LIMIT:
        lines.append(f"::error title=pytest::{len(found) - LIMIT} more failed tests")
    return lines


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: ci_test_annotations.py <junit.xml>", file=sys.stderr)
        return 2
    report = Path(argv[1])
    if not report.is_file():
        print(f"::error title=pytest::no report at {report}: pytest did not run or finish")
        return 0
    for line in workflow_commands(failures(report)):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
