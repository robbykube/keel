# Build the Raspberry Pi card image from Windows, using the Debian WSL distro.
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$wslRepo = wsl -d Debian -u root -- wslpath -a $repo
if ($LASTEXITCODE -ne 0) { throw "Debian WSL could not see the Keel folder." }
$wslRepo = $wslRepo.Trim()
wsl -d Debian -u root -- bash -lc "sed -i 's/\r$//' '$wslRepo/packaging/baremetal/build-pi-image.sh' '$wslRepo/packaging/baremetal/pi/first-boot' '$wslRepo/packaging/baremetal/pi/grow-root' && bash '$wslRepo/packaging/baremetal/build-pi-image.sh'"
if ($LASTEXITCODE -ne 0) { throw "The Pi image build failed." }
