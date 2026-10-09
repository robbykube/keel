[CmdletBinding()]
param(
    [switch]$Prepare,
    [string]$Distro = 'Debian'
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$linuxDrive = $projectRoot.Substring(0, 1).ToLowerInvariant()
$linuxPath = $projectRoot.Substring(2).Replace('\', '/')
$linuxRoot = "/mnt/$linuxDrive$linuxPath"

Write-Host 'Checking WSL2...' -ForegroundColor Cyan
$distros = @(wsl.exe -l -q 2>$null)
if ($LASTEXITCODE -ne 0 -or $distros.Count -eq 0) {
    Write-Error @"
No usable WSL distribution was found.

Install WSL2 with a Debian or Ubuntu distribution, restart Windows if asked,
then run this script again. This script does not install WSL or change disks.
"@
}

$arguments = @('--distribution', $Distro, '--user', 'root', '--cd', $linuxRoot, 'bash', 'packaging/baremetal/build-image.sh')
if ($Prepare) {
    $arguments += '--prepare'
}

Write-Host 'Building Keel inside WSL2. This only creates files under dist/baremetal.' -ForegroundColor Green
& wsl.exe @arguments
if ($LASTEXITCODE -ne 0) {
    throw "Keel image build failed with exit code $LASTEXITCODE."
}

Write-Host "Finished. Look under $projectRoot\dist\baremetal\release" -ForegroundColor Green
