#!/usr/bin/env bash
# A working copy of its own for one task (AGENTS.md, "Working copies"): a
# sibling folder <repository>-<topic> on a new branch <kind>/<topic> taken
# from origin/main, so agents working at once never switch each other's
# branch. Run it from any folder of the repository.
#
#   tools/new_worktree.sh <topic> [feature|fix] [--no-sync]
#
# <topic> is kebab-case (blocks-palette). --no-sync skips `uv sync` in the
# new folder.
set -euo pipefail

usage() {
  echo "usage: tools/new_worktree.sh <topic> [feature|fix] [--no-sync]" >&2
  echo "       <topic> in kebab-case, such as blocks-palette" >&2
  exit 2
}

topic=${1:-}
[[ -n $topic ]] || usage
shift
kind=feature
sync=1
for arg in "$@"; do
  case "$arg" in
    feature | fix) kind=$arg ;;
    --no-sync) sync=0 ;;
    *) usage ;;
  esac
done
[[ $topic =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]] || usage

# Next to the main folder, named after it, even when run from a worktree.
main_root=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
target="$(dirname "$main_root")/$(basename "$main_root")-$topic"
branch="$kind/$topic"
if [[ -e $target ]]; then
  echo "$target already exists" >&2
  exit 1
fi
if git show-ref --verify --quiet "refs/heads/$branch"; then
  echo "branch $branch already exists" >&2
  exit 1
fi

git fetch --quiet origin
git worktree add --quiet "$target" -b "$branch" origin/main
if ((sync)); then
  (cd "$target" && uv sync)
fi
echo "Working copy ready: $target on $branch"
