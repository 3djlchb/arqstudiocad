"""The CI workflows keep ADR 0037's hardening: a read-only token, every
action pinned to a commit, checkouts without stored credentials, the Python
audit, and Dependabot keeping the pins current."""

import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
GITHUB = ROOT / ".github"
WORKFLOWS = sorted((GITHUB / "workflows").glob("*.yml"))

#: ``uses: owner/repo@<40 hex digits>  # vX.Y.Z``
PINNED = re.compile(r"^\s*- uses: [\w.-]+/[\w./-]+@[0-9a-f]{40}  # v\d+\.\d+\.\d+$")


def test_there_are_workflows_to_check() -> None:
    assert {w.name for w in WORKFLOWS} >= {"ci.yml", "packages.yml"}


@pytest.mark.parametrize("workflow", WORKFLOWS, ids=lambda w: w.name)
def test_the_token_only_reads(workflow: Path) -> None:
    text = workflow.read_text(encoding="utf-8")
    assert re.search(r"(?m)^permissions:\n  contents: read\n", text), "top-level read-only token"
    assert not re.search(r"(?m)^\s+\w[\w-]*: write$", text), "no job asks for write access"


@pytest.mark.parametrize("workflow", WORKFLOWS, ids=lambda w: w.name)
def test_every_action_is_pinned_to_a_commit(workflow: Path) -> None:
    uses = [line for line in workflow.read_text(encoding="utf-8").splitlines() if "uses:" in line]
    assert uses, "a workflow with no actions checks nothing"
    loose = [line.strip() for line in uses if not PINNED.match(line)]
    assert not loose, f"pin these to a commit SHA, release in a comment: {loose}"


@pytest.mark.parametrize("workflow", WORKFLOWS, ids=lambda w: w.name)
def test_checkouts_keep_no_credentials(workflow: Path) -> None:
    lines = workflow.read_text(encoding="utf-8").splitlines()
    checkouts = [i for i, line in enumerate(lines) if "- uses: actions/checkout@" in line]
    assert checkouts
    for i in checkouts:
        assert lines[i + 1].strip() == "with:", lines[i]
        assert lines[i + 2].strip() == "persist-credentials: false", lines[i]


def test_ci_audits_every_locked_python_package() -> None:
    text = (GITHUB / "workflows" / "ci.yml").read_text(encoding="utf-8")
    assert "uv export --locked --no-emit-project" in text
    assert re.search(r"uvx pip-audit@\d+\.\d+\.\d+ --strict --require-hashes", text)


def test_dependabot_keeps_the_pinned_actions_current() -> None:
    text = (GITHUB / "dependabot.yml").read_text(encoding="utf-8")
    assert "package-ecosystem: github-actions" in text


def test_the_security_policy_says_how_to_report() -> None:
    text = (ROOT / "SECURITY.md").read_text(encoding="utf-8")
    assert "security/advisories/new" in text
    assert "## Principles for what is not built yet" in text
