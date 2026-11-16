set -uo pipefail
cfg=/var/lib/technitium
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/names.json.tmp
  mv /run/keel/names.json.tmp /run/keel/names.json
}
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
page_ok() {
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "$1" || true)
  case "$code" in
    2*|3*) return 0 ;;
  esac
  return 1
}
port_ok() {
  timeout 2 bash -c "echo >/dev/tcp/$1/$2" >/dev/null 2>&1
}
ready() {
  page_ok "http://127.0.0.1:5380/" && page_ok "http://$house:5380/" && port_ok "$house" 53
}
# The saved setup listens on this computer. Tell it the house address too,
# then start it again so that address is the one that answers.
open_house() {
  mark 92 "Telling Names the house address."
  login=$(curl -sS -X POST "http://127.0.0.1:5380/api/user/login" --data "user=admin&pass=admin" || true)
  token=$(printf '%s' "$login" | sed -n 's/.*"token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
  [ -n "$token" ] || return 1
  setreply=$(curl -sS -X POST "http://127.0.0.1:5380/api/settings/set" \
    --data-urlencode "token=$token" \
    --data-urlencode "webServiceLocalAddresses=${house},127.0.0.1" \
    --data-urlencode "dnsServerLocalEndPoints=${house}:53,127.0.0.1:53" || true)
  printf '%s' "$setreply" | grep -Eq '"status"[[:space:]]*:[[:space:]]*"ok"' || return 1
  podman restart keel-names >"$log" 2>&1 || return 1
  return 0
}
wait_ready() {
  tries=$1
  i=0
  while [ "$i" -lt "$tries" ]; do
    ready && finish
    i=$((i + 1))
    sleep 2
  done
  return 1
}
finish() {
  cat > /etc/systemd/system/keel-names.service <<'EOF'
[Unit]
Description=Keel names
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-names
ExecStop=/usr/bin/podman stop -t 10 keel-names
[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload >"$log" 2>&1 || true
  systemctl enable keel-names.service >"$log" 2>&1 || true
  touch "$cfg/installed"
  printf '%s\n' "$house"
  exit 0
}
mark 8 "Opening Names."
install -d -m 0755 "$cfg/config" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 18 "Installing the program that answers names."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Names. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install Names. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so Names did not start."
image=docker.io/technitium/dns-server:15.6.2
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 45 "Downloading Names. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Names. Check the network and try again."
else
  mark 45 "Opening Names."
fi
mark 75 "Starting Names."
if podman container exists keel-names >/dev/null 2>&1; then
  podman start keel-names >"$log" 2>&1 || true
  wait_ready 15 || true
  if page_ok "http://127.0.0.1:5380/"; then
    open_house && wait_ready 40 || true
  fi
fi
# The last setup answered on this computer and not on the house address.
# Open kept it, so the next start does too. Begin again when the house
# address is still closed.
rm -f "$cfg/installed"
podman rm -f keel-names >"$log" 2>&1 || true
rm -rf "$cfg/config"
install -d -m 0755 "$cfg/config"
if ! podman run -d --name keel-names --hostname keel-names --network host \
  -e DNS_SERVER_DOMAIN=keel \
  -e DNS_SERVER_WEB_SERVICE_LOCAL_ADDRESSES=0.0.0.0 \
  -e TZ=UTC \
  -v "$cfg/config:/etc/dns" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering names. Keel left that in place."
  fi
  fail "Keel could not start Names. Try Open again."
fi
mark 90 "Waiting for the names page."
i=0
while [ "$i" -lt 40 ]; do
  page_ok "http://127.0.0.1:5380/" && break
  i=$((i + 1))
  sleep 2
done
if ! podman inspect -f '{{.State.Running}}' keel-names 2>/dev/null | grep -qx true; then
  last=$(podman logs --tail 20 keel-names 2>&1 | sed '/^$/d' | tail -n 1 | cut -c 1-160)
  fail "${last:-Names stopped before the page was ready. Try Open again.}"
fi
page_ok "http://127.0.0.1:5380/" || fail "Names started, but the page did not answer. Try Open again."
open_house || fail "Keel could not tell Names the house address. Try Open again."
wait_ready 40 || true
if page_ok "http://$house:5380/" && ! port_ok "$house" 53; then
  fail "The names page is open, but this computer is not answering lookups yet. Try Open again."
fi
if page_ok "http://127.0.0.1:5380/"; then
  fail "Names is open on this computer, but the house address did not answer. Try Open again."
fi
last=$(podman logs --tail 40 keel-names 2>&1 | sed '/^$/d' | tail -n 1 | cut -c 1-140)
fail "Names started, but the page did not answer. ${last:-Try Open again.}"
