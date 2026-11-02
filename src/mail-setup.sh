set -uo pipefail
host='__KEEL_HOST__'
cfg=/var/lib/thunderbird
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/mail.json.tmp
  mv /run/keel/mail.json.tmp /run/keel/mail.json
}
mark 8 "Opening Mail."
install -d -m 0755 "$cfg/config" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1 || ! command -v openssl >/dev/null 2>&1; then
  mark 16 "Installing the program that keeps mail."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Mail. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl openssl >"$log" 2>&1 || fail "Keel could not install Mail. Try Open again."
fi
if [ -f "$cfg/page.pass" ]; then
  pagepass=$(cat "$cfg/page.pass")
else
  pagepass=$(openssl rand -hex 12)
  umask 077
  printf '%s\n' "$pagepass" > "$cfg/page.pass"
  chmod 0600 "$cfg/page.pass"
fi
[ -n "$pagepass" ] || fail "Keel could not make the Mail sign-in."
image=lscr.io/linuxserver/thunderbird:1140.17.0
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 50 "Downloading Mail. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Mail. Check the network and try again."
else
  mark 50 "Opening Mail."
fi
mark 75 "Starting Mail."
podman network exists keel-mail >/dev/null 2>&1 || podman network create keel-mail >"$log" 2>&1 || fail "Keel could not start Mail. Try Open again."
podman rm -f keel-mail >"$log" 2>&1 || true
if ! podman run -d --name keel-mail --hostname mail \
  --network keel-mail --network-alias mail \
  --shm-size=1g \
  -e PUID=0 \
  -e PGID=0 \
  -e TZ=UTC \
  -e CUSTOM_USER=mail \
  -e PASSWORD="$pagepass" \
  -p 127.0.0.1:3000:3000 \
  -v "$cfg/config:/config" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on that address. Mail did not start."
  fi
  fail "Keel could not start Mail. Try Open again."
fi
mark 90 "Waiting for the mail page."
up=0
i=0
while [ "$i" -lt 60 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:3000/" || true)
  case "$code" in
    2*|3*|401) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = 1 ] || fail "Mail started, but the page did not answer. Try Open again."
podman network connect keel-mail keel-door >"$log" 2>&1 || true
cat > /etc/systemd/system/keel-mail.service <<'EOF'
[Unit]
Description=Keel mail
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-mail
ExecStop=/usr/bin/podman stop -t 20 keel-mail
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-mail.service >"$log" 2>&1 || true
touch "$cfg/installed"
printf 'ready|%s\n' "$pagepass"
exit 0
