set -uo pipefail
cfg=/var/lib/jellyfin
media=/srv/keel/files/Media
log="$cfg/setup.log"
image=docker.io/jellyfin/jellyfin:10.10.7
fail() { echo "$1" >&2; exit 1; }
if [ ! -d "$media" ]; then
  fail "Set up the Files disk first. Films, shows, and music live in Media."
fi
install -d -m 0755 "$cfg" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Watch. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install Watch. Try Open again."
fi
podman image exists "$image" >/dev/null 2>&1 || podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Watch. Check the network and try again."
podman network exists keel-jellyfin >/dev/null 2>&1 || podman network create keel-jellyfin >"$log" 2>&1 || fail "Keel could not start Watch. Try Open again."
podman rm -f keel-jellyfin >/dev/null 2>&1 || true
podman run -d --name keel-jellyfin --memory=2g --restart unless-stopped \
  --network keel-jellyfin \
  -p 127.0.0.1:8096:8096 \
  -v "$cfg:/config" \
  -v "$media:/media:ro" \
  "$image" >"$log" 2>&1 || fail "Keel could not start Watch. Try Open again."
podman network connect keel-jellyfin keel-door >"$log" 2>&1 || true
ok=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 http://127.0.0.1:8096/web/ || true)
  case "$code" in
    2*|3*) ok=1; break ;;
  esac
  sleep 2
done
[ "$ok" = 1 ] || fail "Watch did not answer. Try Open again."
# The first time, Keel finishes Watch's setup itself: one house account and
# one library over Media, so Open signs in and the films are already there.
signin=/var/lib/keel/signin/watch.json
if [ ! -f "$signin" ] && curl -s --max-time 5 http://127.0.0.1:8096/System/Info/Public | grep -q '"StartupWizardCompleted":false'; then
  account=$(openssl rand -hex 12 2>/dev/null || head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n')
  j() { curl -sS -o /dev/null --max-time 15 -w '%{http_code}' -H 'Content-Type: application/json' "$@" || true; }
  j -X POST http://127.0.0.1:8096/Startup/Configuration -d '{"UICulture":"en-US","MetadataCountryCode":"US","PreferredMetadataLanguage":"en"}' >/dev/null
  curl -s -o /dev/null --max-time 15 http://127.0.0.1:8096/Startup/User || true
  j -X POST http://127.0.0.1:8096/Startup/User -d "{\"Name\":\"house\",\"Password\":\"$account\"}" >/dev/null
  j -X POST http://127.0.0.1:8096/Startup/RemoteAccess -d '{"EnableRemoteAccess":true,"EnableAutomaticPortMapping":false}' >/dev/null
  done=$(j -X POST http://127.0.0.1:8096/Startup/Complete)
  case "$done" in
    2*)
      install -d -m 0700 /var/lib/keel/signin
      umask 077
      printf '{"user":"house","password":"%s"}\n' "$account" > "$signin"
      chmod 0600 "$signin"
      auth='MediaBrowser Client="Keel", Device="Keel", DeviceId="keel-setup", Version="1"'
      token=$(curl -sS --max-time 15 -X POST http://127.0.0.1:8096/Users/AuthenticateByName \
        -H 'Content-Type: application/json' -H "Authorization: $auth" \
        -d "{\"Username\":\"house\",\"Pw\":\"$account\"}" | sed -n 's/.*"AccessToken":"\([^"]*\)".*/\1/p')
      if [ -n "$token" ]; then
        curl -sS -o /dev/null --max-time 30 -X POST "http://127.0.0.1:8096/Library/VirtualFolders?name=Media&refreshLibrary=true" \
          -H 'Content-Type: application/json' -H "Authorization: $auth, Token=\"$token\"" \
          -d '{"LibraryOptions":{"PathInfos":[{"Path":"/media"}]}}' || true
      fi
      ;;
  esac
fi
cat > /etc/systemd/system/keel-watch.service <<'EOF'
[Unit]
Description=Keel Watch
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-jellyfin
ExecStop=/usr/bin/podman stop -t 20 keel-jellyfin
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >>"$log" 2>&1 || fail "Keel could not load the Watch service."
systemctl enable keel-watch.service >>"$log" 2>&1 || fail "Keel could not enable Watch to stay on."
podman stop -t 20 keel-jellyfin >>"$log" 2>&1 || true
systemctl restart keel-watch.service >>"$log" 2>&1 || fail "Keel could not keep Watch running. Check Logs."
touch "$cfg/installed"
