<#
.SYNOPSIS
  Create a Hyper-V VM for testing the Keel live ISO on this Windows machine.

.DESCRIPTION
  Hyper-V is the right hypervisor here specifically because WSL2 is in use:
  WSL2 already claims the hypervisor, so VirtualBox and VMware fall back to a
  slow emulation path on the same host.

  A Generation 2 VM gives the guest Hyper-V's synthetic GPU, which Linux 5.14+
  drives with hyperv_drm. That creates /dev/dri/card0, which is what the Keel
  kiosk checks for before starting. So this VM tests the actual appliance
  screen, not just the web UI.

  Secure Boot is disabled because the remastered ISO's boot chain is no longer
  the one Microsoft's UEFI CA signed.

  This script does not touch physical disks. It creates a VHDX under -VmPath.
#>
[CmdletBinding()]
param(
  [string]$Name = 'keel-test',
  # Resolved below rather than here: a '..\..' default is built before the
  # working directory is known and trips path validation.
  [string]$IsoPath,
  [string]$VmPath = 'E:\keel-vms',
  [int64]$MemoryBytes = 4GB,
  [int64]$DiskBytes = 32GB,
  [int]$CpuCount = 2,
  [string]$SwitchName = 'Default Switch'
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw 'Hyper-V management needs an elevated PowerShell. Run this as Administrator.'
}

if (-not $IsoPath) {
  # packaging\baremetal -> packaging -> repo root, without walking through '..'.
  $repoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
  $IsoPath = Join-Path $repoRoot 'dist\baremetal\release\keel-amd64-install.iso'
}

if (-not (Test-Path -LiteralPath $IsoPath)) {
  throw "No ISO at $IsoPath. Build it first: wsl -d Debian -u root -- bash packaging/baremetal/remaster-official.sh"
}
$IsoPath = (Resolve-Path -LiteralPath $IsoPath).Path

if (Get-VM -Name $Name -ErrorAction SilentlyContinue) {
  throw "A VM named '$Name' already exists. Remove it first, or pass -Name for a second one."
}

if (-not (Get-VMSwitch -Name $SwitchName -ErrorAction SilentlyContinue)) {
  throw "No Hyper-V switch named '$SwitchName'. List them with Get-VMSwitch."
}

if (-not (Test-Path $VmPath)) { New-Item -ItemType Directory -Path $VmPath | Out-Null }
$vhd = Join-Path $VmPath "$Name.vhdx"
if (Test-Path $vhd) { throw "A disk already exists at $vhd. Remove it or pass a different -VmPath." }

New-VM -Name $Name -Generation 2 -MemoryStartupBytes $MemoryBytes `
  -NewVHDPath $vhd -NewVHDSizeBytes $DiskBytes -SwitchName $SwitchName | Out-Null

Set-VMProcessor -VMName $Name -Count $CpuCount
Set-VMFirmware -VMName $Name -EnableSecureBoot Off
Add-VMDvdDrive -VMName $Name -Path $IsoPath
Set-VMFirmware -VMName $Name -FirstBootDevice (Get-VMDvdDrive -VMName $Name)

# The kiosk repaints on its own; checkpoints just fill the disk during testing.
Set-VM -Name $Name -AutomaticCheckpointsEnabled $false

Write-Host ""
Write-Host "Created VM '$Name'" -ForegroundColor Green
Write-Host "  ISO:    $IsoPath"
Write-Host "  Disk:   $vhd ($([math]::Round($DiskBytes/1GB)) GB, empty - the live ISO does not install to it)"
Write-Host "  Switch: $SwitchName"
Write-Host ""
Write-Host "Start it and open the console with:"
Write-Host "  Start-VM $Name; vmconnect.exe localhost $Name"
Write-Host ""
Write-Host "The Keel GUI should fill the VM console. To reach it from this desktop's"
Write-Host "browser instead, find the guest address on the recovery console (Ctrl+Alt+F3)"
Write-Host "with 'hostname -I' and open http://<address>:9478/"
