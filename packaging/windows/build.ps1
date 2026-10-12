<#
.SYNOPSIS
  Build the Windows package of ArqStudioCAD (ADR 0009).

.DESCRIPTION
  1. Release wheel of the Rust extension (maturin).
  2. Clean packaging environment with the bundled Python version.
  3. PyInstaller onedir bundle (packaging/arqstudiocad.spec).
  4. Self-check of the bundle (arqstudiocad-cli --check); a failing bundle stops here.
  5. Inno Setup installer.

  Output goes to dist\. PyInstaller cannot cross-compile: build x64 on x64
  Windows and arm64 on ARM64 Windows.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File packaging\windows\build.ps1
#>
param(
    [switch]$SkipInstaller,
    [string]$Python = "3.13"
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $Root
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }

function Invoke-Native {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Command)
    & $Command[0] $Command[1..($Command.Length - 1)]
    if ($LASTEXITCODE -ne 0) { throw "Command failed ($LASTEXITCODE): $($Command -join ' ')" }
}

foreach ($dir in "build\wheels", "build\pkg-venv", "build\pyinstaller", "dist") {
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
}

Write-Host "==> 1/5 Release wheel" -ForegroundColor Cyan
Invoke-Native uv run maturin build --release --out build\wheels
$Wheel = (Get-ChildItem build\wheels\*.whl | Select-Object -First 1).FullName

Write-Host "==> 2/5 Packaging environment (Python $Python)" -ForegroundColor Cyan
Invoke-Native uv venv build\pkg-venv --python $Python
$Py = "$Root\build\pkg-venv\Scripts\python.exe"
Invoke-Native uv pip install --python $Py $Wheel "pyinstaller>=6.22,<7"

$Meta = & $Py -c "from arqstudiocad import __version__ as v; from arqstudiocad.product import APP_NAME as n, APP_ID as i; print(n); print(i); print(v)"
if ($LASTEXITCODE -ne 0) { throw "Cannot read product metadata" }
$AppName, $AppId, $Version = $Meta

Write-Host "==> 3/5 PyInstaller ($AppName $Version, $Arch)" -ForegroundColor Cyan
Invoke-Native $Py -m PyInstaller packaging\arqstudiocad.spec --noconfirm --distpath dist --workpath build\pyinstaller

Write-Host "==> 4/5 Bundle self-check" -ForegroundColor Cyan
Invoke-Native "dist\$AppName\arqstudiocad-cli.exe" --check

if ($SkipInstaller) { Write-Host "Bundle ready: dist\$AppName"; return }

Write-Host "==> 5/5 Inno Setup installer" -ForegroundColor Cyan
$Iscc = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $Iscc) { throw "Inno Setup 6 not found: winget install --source winget --id JRSoftware.InnoSetup" }

Invoke-Native $Iscc /Q `
    "/DAppName=$AppName" "/DAppId=$AppId" "/DAppVersion=$Version" "/DArch=$Arch" `
    "/DSourceDir=$Root\dist\$AppName" "/DOutputDir=$Root\dist" "/DRepoRoot=$Root" `
    packaging\windows\arqstudiocad.iss

Get-ChildItem dist\*.exe | ForEach-Object { Write-Host ("Installer: {0} ({1:N0} MB)" -f $_.FullName, ($_.Length / 1MB)) }
