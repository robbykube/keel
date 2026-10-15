# Keel implementation plan

## Product boundary

Keel is a self-hosted-first appliance OS with a curated application layer. The OS owns identity, networking, storage, service lifecycle, health, updates, and recovery. Applications are installed and managed through Keel; they are not required to run continuously.

The first release is not a general Linux distribution, a hypervisor, or an app-store marketplace.

## Non-negotiable principles

- User data lives on an explicit persistent data volume, separate from the system image.
- Local LAN operation must work without the overlay.
- The overlay is provider-neutral in Keel's data model.
- User-facing policy and power state use `overlay`, never a provider-specific brand.
- Headscale is the preferred self-hosted provider; Tailscale is the compatibility/default path during early development.
- SMB and the GUI must expose and test both LAN and overlay paths.
- Core services are native systemd services. Containers are an app isolation mechanism, not the OS architecture.
- Mutations are declarative, validated, auditable, and reversible where possible.
- No destructive storage action is one click.
- Normal operation is GUI-first; CLI/SSH/console are bootstrap and recovery paths.

## Milestone 0 — control-plane foundation

- Introduce an overlay provider abstraction.
- Add persistent, versioned Keel configuration.
- Add a real state directory and atomic writes.
- Add admin authentication and safe defaults before LAN deployment.
- Bind to loopback by default; require explicit LAN exposure during development.
- Transitional bearer-token seam: `KEEL_ADMIN_TOKEN` protects mutations; `/login.html` exchanges it for a short-lived HttpOnly browser session. Non-loopback startup fails closed without it.
- `keelctl bootstrap` now automates Windows-to-Linux SSH installation of the binary, service, state directory, admin environment, and initial start.
- Add Linux command execution boundaries and structured evidence.
- Add integration-test seams for systemd, networking, mounts, and disks.
- Read-only core systemd status now comes from a fixed unit allowlist at `GET /api/services`.
- `GET /api/network/ports` probes the GUI, SSH, and SMB ports; Repair restart requests use a fixed systemd unit allowlist.

## Milestone 1 — Linux bootable appliance

- Build a minimal Debian Stable image with a fixed package manifest.
- Install `keel.service` as the always-on control plane.
- Keep system and data partitions separate.
- Add first-boot setup: hostname, admin, network, overlay, and data location.
- Provide SSH recovery access and a documented rescue path.
- Primary deployment is a whole dedicated computer; VM remains the disposable
  development and CI path.
- First installation transport is a Windows-written USB image. PXE/iPXE and
  BMC virtual media follow once USB installation is reliable.
- Bare-metal image scaffolding and package manifest live under
  `packaging/baremetal/`; the current build script is build-host-only and
  never writes a target disk.

## Milestone 2 — self-hosted overlay

- Model an `OverlayProvider` with `headscale`, `tailscale`, and future providers.
- Implement Headscale enrollment/status through the local Tailscale-compatible client.
- Keep Headscale control-plane hosting independent from the user's only data disk where practical.
- Display provider, node identity, LAN address, overlay address, DNS name, and connectivity tests.
- Add an optional Headscale deployment recipe with TLS, backup, upgrade, and recovery guidance.
- Initial deployment artifacts live under `packaging/`: a native systemd unit contract and a Headscale placement/enrollment guide.

## Milestone 3 — storage and files

- Implement foreign NFS/SMB mounts first; avoid taking ownership of existing ZFS pools.
- Add local filesystem/pool support after the foreign-storage path is reliable.
- Persist mount and pool intent separately from observed health.
- Implement Samba shares with users/groups, read-only policy, path validation, and lifecycle operations.
- Verify each share over LAN and overlay before marking it healthy.

## Milestone 4 — app lifecycle

- Define a versioned Keel app manifest.
- Versioned app manifests now expose runtime, ports, dependencies, data paths, healthcheck, and idle policy at `GET /api/apps`.
- `POST /api/apps/{id}/action` provides an allowlisted start/stop/restart/park seam backed by `keel-app@id.service` on Linux.
- Support native systemd apps and controlled OCI apps behind one lifecycle API.
- Model dependencies, ports, volumes, secrets, health checks, backup paths, and idle policy.
- Install one reference app end-to-end before expanding the catalog.
- Add start, stop, restart, update, rollback, logs, and repair operations.

## Milestone 5 — health, updates, and recovery

- Replace mock repair actions with systemd, mount, port, firewall, SMART, and overlay evidence.
- Add signed system/app artifacts and staged updates.
- Add system rollback without touching user data.
- Add snapshots, scrub scheduling, backup targets, and a simple restore flow.

## Milestone 6 — power and catalog

- Keep Keel, SSH, storage dependencies, Samba, and the selected overlay floor available in Idle.
- Stop or park unused apps based on declared policy and measured activity.
- Add CPU, disk I/O, job, and app-health evidence to advisor decisions.
- Expand the curated catalog only when each app has a complete Keel contract.

## Immediate build slice

The box is installed and still on 0.1.5. Signed v0.1.7 rolled back from Settings.

1. The repair image is built (2026-10-15, SHA-256 `c0102c3fdaded192a3716ba2e47748112007d15d0b179fbfd1a391ea557f0814`). Copy it to Ventoy and boot it once. Leave `/var/lib/keel` alone.
2. On the box: give the data disks their jobs, then set one folder name.
3. In the code: stop returning the folder password, require a session for admin reads off the Keel screen, and escape the remaining pages.
4. After the folders are on the data disks: SMART and free space, then the house-network firewall.

## Definition of the first real demo

On a disposable Linux machine, a user can boot Keel, create an admin, enroll in Headscale, attach or mount storage, create an SMB share, open it from LAN and overlay, install one app, stop it, restart it, and recover from one failed service without editing configuration files manually.

## Proxmox development path

The safe Windows-to-Linux test path is a disposable VM on Proxmox, not a
partition or installation on the Proxmox host itself. Start with one Keel VM
using a virtual OS disk and a separate virtual data disk, bridged to an
isolated test LAN where possible. Use snapshots before storage and update
tests. Add no HBA or physical-disk passthrough until native storage behavior
is intentionally being tested on disposable hardware.

For overlay testing, use a separate small Headscale VM/LXC or an existing
non-production test control plane. The Windows browser is the normal admin
client; SSH and the Proxmox console are bootstrap/recovery only.
