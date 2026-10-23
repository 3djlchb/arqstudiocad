<#
A working copy of its own for one task (AGENTS.md, "Working copies"): a
sibling folder <repository>-<topic> on a new branch <kind>/<topic> taken
from origin/main, so agents working at once never switch each other's
branch. Run it from any folder of the repository.

    tools\new_worktree.ps1 <topic> [-Kind feature|fix] [-NoSync]

<topic> is kebab-case (blocks-palette). -NoSync skips `uv sync` in the new
folder. Works in Windows PowerShell 5.1 and PowerShell 7.
#>
param(
    [string]$Topic = '',
    [string]$Kind = 'feature',
    [switch]$NoSync
)

function Stop-WithUsage {
    [Console]::Error.WriteLine('usage: tools\new_worktree.ps1 <topic> [-Kind feature|fix] [-NoSync]')
    [Console]::Error.WriteLine('       <topic> in kebab-case, such as blocks-palette')
    exit 2
}

if ($Topic -cnotmatch '^[a-z0-9]+(-[a-z0-9]+)*$') { Stop-WithUsage }
if ($Kind -cne 'feature' -and $Kind -cne 'fix') { Stop-WithUsage }

# Next to the main folder, named after it, even when run from a worktree.
$first = git worktree list --porcelain | Select-Object -First 1
$mainRoot = $first -replace '^worktree ', ''
$target = Join-Path (Split-Path $mainRoot -Parent) ((Split-Path $mainRoot -Leaf) + "-$Topic")
$branch = "$Kind/$Topic"
if (Test-Path -LiteralPath $target) {
    [Console]::Error.WriteLine("$target already exists")
    exit 1
}
git show-ref --verify --quiet "refs/heads/$branch"
if ($LASTEXITCODE -eq 0) {
    [Console]::Error.WriteLine("branch $branch already exists")
    exit 1
}

git fetch --quiet origin
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git worktree add --quiet $target -b $branch origin/main
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not $NoSync) {
    Push-Location -LiteralPath $target
    try {
        uv sync
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    finally {
        Pop-Location
    }
}
Write-Output "Working copy ready: $target on $branch"
