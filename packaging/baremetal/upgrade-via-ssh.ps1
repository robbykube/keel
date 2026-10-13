param(
  [Parameter(Mandatory = $true)]
  [string]$Target,
  [string]$Version = "0.1.5"
)

$ErrorActionPreference = "Stop"

if ($Target -match '[\s''";|&<>`$]') {
  throw "Target must look like user@address and contain no shell metacharacters."
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
  throw "GitHub CLI (gh) is required and must already be signed in."
}
if (-not (Get-Command scp -ErrorAction SilentlyContinue)) {
  throw "OpenSSH scp is required."
}
if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) {
  throw "OpenSSH ssh is required."
}
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
  throw "tar is required to unpack the release."
}

$release = "v$Version"
$work = Join-Path ([System.IO.Path]::GetTempPath()) "keel-upgrade-$Version"
$archive = Join-Path $work "keel-linux-amd64-$release.tar.gz"
$package = Join-Path $work "keel"
$remoteBinary = "/tmp/keel-upgrade-$Version"
$remoteWeb = "/tmp/keel-web-upgrade-$Version"

Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $work | Out-Null

Write-Host "Downloading Keel $release..."
gh release download $release --repo robbykube/keel --pattern "keel-linux-amd64-$release.tar.gz" --dir $work --clobber
tar -xzf $archive -C $work

Write-Host "Copying Keel to $Target..."
scp (Join-Path $package "keel") "${Target}:$remoteBinary"
scp -r (Join-Path $package "web") "${Target}:$remoteWeb"

Write-Host "Installing Keel without touching data or settings..."
ssh -t $Target "sudo install -m 0755 $remoteBinary /usr/bin/keel && sudo install -d -m 0755 /usr/lib/keel/web && sudo cp -a $remoteWeb/. /usr/lib/keel/web/ && sudo systemctl restart keel.service"

Write-Host "Done. Check http://<keel-ip>:9478/api/health for version $Version."
