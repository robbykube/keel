set -uo pipefail
cfg=/var/lib/npm
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/door.json.tmp
  mv /run/keel/door.json.tmp /run/keel/door.json
}
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
mark 8 "Opening the front door."
install -d -m 0755 "$cfg/data" "$cfg/letsencrypt" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 18 "Installing the program that answers a name."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install the front door. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so the front door did not start."
image=docker.io/jc21/nginx-proxy-manager:2.16.0
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 45 "Downloading the front door. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
else
  mark 45 "Opening the front door."
fi
mark 75 "Starting the front door."
podman rm -f keel-door >"$log" 2>&1 || true
if ! podman run -d --name keel-door --hostname keel-door \
  -p "$house:80:80" -p "$house:443:443" -p "$house:81:81" \
  -e DISABLE_IPV6=true \
  -e TZ=UTC \
  -v "$cfg/data:/data" \
  -v "$cfg/letsencrypt:/etc/letsencrypt" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on port 80 or 443. The front door did not start."
  fi
  fail "Keel could not start the front door. Try Open again."
fi
mark 90 "Waiting for the front door."
up=0
i=0
while [ "$i" -lt 40 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://$house:81/" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = "1" ] || fail "The front door started, but the page did not answer. Try Open again."
cat > /etc/systemd/system/keel-door.service <<'EOF'
[Unit]
Description=Keel front door
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-door
ExecStop=/usr/bin/podman stop -t 10 keel-door
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-door.service >"$log" 2>&1 || true
if podman container exists keel-photos-server >/dev/null 2>&1; then
  podman network connect keel-photos keel-door >"$log" 2>&1 || true
fi
if podman container exists keel-jitsi-web >/dev/null 2>&1; then
  podman network connect keel-jitsi keel-door >"$log" 2>&1 || true
fi
if podman container exists keel-passwords >/dev/null 2>&1; then
  podman network connect keel-passwords keel-door >"$log" 2>&1 || true
fi
if podman container exists keel-papers >/dev/null 2>&1; then
  podman network connect keel-papers keel-door >"$log" 2>&1 || true
fi
if podman container exists keel-mail >/dev/null 2>&1; then
  podman network connect keel-mail keel-door >"$log" 2>&1 || true
fi
touch "$cfg/installed"
printf '%s\n' "$house"
exit 0
