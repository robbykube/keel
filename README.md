# keel

Opinionated NAS appliance with a **web GUI**, not a TUI.

The point of building our own is control over **efficiency and speed**: one small daemon, a dense browser UI, short path from disk to share. No Electron, no desktop session on the NAS, no plugin bazaar in v1.

## Principles

1. Appliance, not a ricing distro. Defaults should be enough.
2. Admin is a browser on the LAN, like Proxmox.
3. Measure: daemon RSS, API latency, page weight.
4. Linux is the host. Windows is for UI/API development with mock disks.

## Run

```bash
cargo run
```

Open http://127.0.0.1:9478

The development server binds to loopback by default. Set `KEEL_BIND` explicitly when testing from another machine; LAN exposure will require the appliance authentication flow before production use.

For development, set `KEEL_ADMIN_TOKEN` to protect state-changing API calls with `Authorization: Bearer <token>`. Non-loopback binding refuses to start without this token. Visit `/login.html` to exchange the token for a short-lived HttpOnly browser session.

To bootstrap a Linux VM from Windows after building a Linux Keel binary:

```text
cargo run --bin keelctl -- bootstrap --host user@192.168.1.50 --binary target/x86_64-unknown-linux-gnu/release/keel --admin-token YOUR_TOKEN
```

`keelctl` uses the Windows OpenSSH client, installs the binary and `keel.service`, creates the Keel state directory, starts the service, and prints the GUI URL. The remote account must have non-interactive `sudo` for the bootstrap operation.

## Status

v0: dashboard, disk inventory, stub pools/shares. Storage mutations come next.
