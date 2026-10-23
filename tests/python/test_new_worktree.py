"""tools/new_worktree: each task in a working copy of its own, on a new
branch from origin/main, so agents working at once never switch each other's
branch (AGENTS.md, "Working copies"). Run in a scratch repository with its
own origin."""

import os
import shutil
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

TOOLS = Path(__file__).resolve().parents[2] / "tools"
CONFIG = Path(__file__).resolve().parents[2] / ".pre-commit-config.yaml"

Runner = Callable[..., subprocess.CompletedProcess[str]]

GIT_ENV = {
    **os.environ,
    "GIT_AUTHOR_NAME": "Test",
    "GIT_AUTHOR_EMAIL": "test@example.com",
    "GIT_COMMITTER_NAME": "Test",
    "GIT_COMMITTER_EMAIL": "test@example.com",
}


def git(cwd: Path, *args: str) -> str:
    done = subprocess.run(
        ["git", *args], cwd=cwd, env=GIT_ENV, capture_output=True, text=True, check=True
    )
    return done.stdout.strip()


def git_bash() -> str | None:
    """Git's own bash: on Windows `bash` may be WSL's, which sees other paths."""
    if sys.platform != "win32":
        return shutil.which("bash")
    found = shutil.which("git")
    if found is None:
        return None
    for candidate in Path(found).resolve().parents:
        bash = candidate / "bin" / "bash.exe"
        if bash.is_file():
            return str(bash)
    return None


def powershell() -> str | None:
    return shutil.which("powershell") or shutil.which("pwsh")


def bash_runner(cwd: Path, *args: str) -> subprocess.CompletedProcess[str]:
    bash = git_bash()
    assert bash is not None
    script = (TOOLS / "new_worktree.sh").as_posix()
    return subprocess.run(
        [bash, script, *args], cwd=cwd, env=GIT_ENV, capture_output=True, text=True, check=False
    )


def powershell_runner(cwd: Path, *args: str) -> subprocess.CompletedProcess[str]:
    shell = powershell()
    assert shell is not None
    # The bash spelling of the options, in PowerShell's.
    translated = [
        {"fix": "-Kind", "feature": "-Kind", "--no-sync": "-NoSync"}.get(a, a) for a in args
    ]
    command: list[str] = []
    for original, word in zip(args, translated, strict=True):
        command += [word, original] if word == "-Kind" else [word]
    script = str(TOOLS / "new_worktree.ps1")
    return subprocess.run(
        [shell, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, *command],
        cwd=cwd,
        env=GIT_ENV,
        capture_output=True,
        text=True,
        check=False,
    )


RUNNERS = [
    pytest.param(
        bash_runner,
        id="bash",
        marks=pytest.mark.skipif(git_bash() is None, reason="no bash"),
    ),
    pytest.param(
        powershell_runner,
        id="powershell",
        marks=pytest.mark.skipif(powershell() is None, reason="no PowerShell"),
    ),
]


@pytest.fixture
def repository(tmp_path: Path) -> Path:
    """A clone of a scratch origin with one commit on main."""
    origin = tmp_path / "origin.git"
    git(tmp_path, "init", "--quiet", "--bare", "--initial-branch=main", str(origin))
    clone = tmp_path / "app"
    git(tmp_path, "clone", "--quiet", str(origin), str(clone))
    git(clone, "switch", "--quiet", "-c", "main")
    (clone / "readme.txt").write_text("first\n", encoding="utf-8")
    git(clone, "add", "readme.txt")
    git(clone, "commit", "--quiet", "-m", "first")
    git(clone, "push", "--quiet", "origin", "main")
    return clone


@pytest.mark.parametrize("run", RUNNERS)
def test_a_task_gets_a_folder_and_a_branch_of_its_own(run: Runner, repository: Path) -> None:
    done = run(repository, "blocks-palette", "--no-sync")
    assert done.returncode == 0, done.stderr
    target = repository.parent / "app-blocks-palette"
    assert git(target, "branch", "--show-current") == "feature/blocks-palette"
    assert git(target, "rev-parse", "HEAD") == git(repository, "rev-parse", "origin/main")
    assert git(repository, "branch", "--show-current") == "main", "the main folder stays"
    # From inside a working copy, the next one still sits by the main folder.
    done = run(target, "document-roundtrip", "fix", "--no-sync")
    assert done.returncode == 0, done.stderr
    second = repository.parent / "app-document-roundtrip"
    assert git(second, "branch", "--show-current") == "fix/document-roundtrip"


@pytest.mark.parametrize("run", RUNNERS)
def test_bad_topics_and_taken_names_are_refused(run: Runner, repository: Path) -> None:
    for topic in ("Blocks_Palette", "blocks palette", "-blocks", ""):
        done = run(repository, topic, "--no-sync") if topic else run(repository)
        assert done.returncode == 2, (topic, done.stderr)
        assert "usage" in done.stderr
    assert run(repository, "blocks", "--no-sync").returncode == 0
    done = run(repository, "blocks", "--no-sync")
    assert done.returncode == 1 and "already exists" in done.stderr, "the folder is taken"
    shutil.rmtree(repository.parent / "app-blocks")
    git(repository, "worktree", "prune")
    done = run(repository, "blocks", "--no-sync")
    assert done.returncode == 1 and "branch feature/blocks" in done.stderr, "the branch is taken"


def test_commits_on_main_are_refused_by_the_hooks() -> None:
    """Every task works on its own branch: a commit on main is a mistake."""
    lines = [line.strip() for line in CONFIG.read_text(encoding="utf-8").splitlines()]
    hook = lines.index("- id: no-commit-to-branch")
    assert lines[hook + 1] == "args: [--branch, main]"
