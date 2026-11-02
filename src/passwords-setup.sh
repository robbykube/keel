set -uo pipefail
host='__KEEL_HOST__'
close='__KEEL_CLOSE__'
cfg=/var/lib/vaultwarden
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false,"signups_open":false}\n' "$1" "$2" > /run/keel/passwords.json.tmp
  mv /run/keel/passwords.json.tmp /run/keel/passwords.json
}
mark 8 "Opening Passwords."
install -d -m 0755 "$cfg/data" /run/keel
if [ "$close" = yes ]; then
  touch "$cfg/signups.closed"
fi
if [ -f "$cfg/signups.closed" ]; then
  signups=false
  word=closed
else
  signups=true
  word=open
fi
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 18 "Installing the program that keeps passwords."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Passwords. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install Passwords. Try Open again."
fi
vault=docker.io/vaultwarden/server:1.37.4
if ! podman image exists "$vault" >/dev/null 2>&1; then
  mark 50 "Downloading Passwords. This can take a while."
  podman pull "$vault" >"$log" 2>&1 || fail "Keel could not download Passwords. Check the network and try again."
else
  mark 50 "Opening Passwords."
fi
mark 75 "Starting Passwords."
podman network exists keel-passwords >/dev/null 2>&1 || podman network create keel-passwords >"$log" 2>&1 || fail "Keel could not start Passwords. Try Open again."
podman rm -f keel-passwords-web keel-passwords >"$log" 2>&1 || true
if ! podman run -d --name keel-passwords --hostname passwords \
  --network keel-passwords --network-alias passwords \
  -e DOMAIN="https://$host" \
  -e SIGNUPS_ALLOWED="$signups" \
  -e SIGNUPS_VERIFY=false \
  -e IP_HEADER=X-Real-IP \
  -e TZ=UTC \
  -p 127.0.0.1:8081:80 \
  -v "$cfg/data:/data" \
  "$vault" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on that address. Passwords did not start."
  fi
  fail "Keel could not start Passwords. Try Open again."
fi
mark 90 "Waiting for the passwords page."
up=0
i=0
while [ "$i" -lt 40 ]; do
  code=$(curl -fsS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:8081/" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = 1 ] || fail "Passwords started, but the page did not answer. Try Open again."
podman network connect keel-passwords keel-door >"$log" 2>&1 || true
cat > /etc/systemd/system/keel-passwords.service <<'EOF'
[Unit]
Description=Keel passwords
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-passwords
ExecStop=/usr/bin/podman stop -t 10 keel-passwords
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-passwords.service >"$log" 2>&1 || true
touch "$cfg/installed"
printf '%s\n' "$word"
exit 0
