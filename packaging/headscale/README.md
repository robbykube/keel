# Headscale with Keel

Headscale is Keel's self-hosted-first overlay control plane. Keel uses the
Tailscale-compatible client on the appliance, pointed at a Headscale server.

## Recommended placement

For one home appliance, the most resilient layout is:

```text
small always-on host or VPS: Headscale + HTTPS
                                  │
                                  │ coordination / policy
                                  ▼
Keel box: tailscaled + files + apps
```

Do not put the only Headscale control server on the same machine and data
disk that it is responsible for reaching. If the Keel box is down, a separate
control host keeps the rest of the tailnet manageable. Headscale does not
carry ordinary file traffic; the clients establish the encrypted data path.

## Native Debian shape

Run Headscale as a dedicated system user and systemd service on a minimal
Debian host. Keep its configuration and database in `/var/lib/headscale`, its
configuration in `/etc/headscale`, and its public endpoint behind HTTPS on
port 443. Back up the database, private keys, policy, and TLS material.

The public endpoint must be stable before enrolling Keel. Use a DNS name such
as `headscale.example.net`; do not enroll production machines against a
temporary IP address.

## Enroll Keel

1. Create a Headscale namespace/user and a short-lived pre-authentication key.
2. In Keel, open **Overlay** and choose **Headscale**.
3. Enter the HTTPS control URL, the one-time enrollment key, and a machine name.
4. Confirm that Keel reports an overlay address and DNS name.
5. Test the GUI and SMB paths from a second enrolled device.
6. Revoke or expire the enrollment key after use.

Keel persists the provider and control URL, never the enrollment key. The
local client remains the data-plane endpoint and must stay running in Keel's
Idle floor.

## Provider compatibility

The Keel data model names this capability `overlay`, not `tailscale`. Tailscale
hosted coordination remains supported for users who do not want to operate
Headscale. Shares and apps should consume the overlay address/status contract,
not provider-specific environment variables.

## Production gates before shipping

- Admin authentication and HTTPS for the Keel GUI
- Headscale backups and documented restore
- ACL policy denying all unintended device-to-device access
- tested revocation of a lost device
- LAN and overlay SMB reachability checks
- a recovery path when the Headscale server is unavailable
