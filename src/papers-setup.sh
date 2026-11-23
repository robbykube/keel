set -uo pipefail
host='__KEEL_HOST__'
cfg=/var/lib/papra
docs=/srv/keel/files/Documents
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/papers.json.tmp
  mv /run/keel/papers.json.tmp /run/keel/papers.json
}
mark 8 "Opening Papers."
if [ ! -d /srv/keel/files ]; then
  fail "Set up the Files disk first. Papers live in Documents."
fi
install -d -m 0755 "$docs" "$cfg/app-data" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1 || ! command -v openssl >/dev/null 2>&1; then
  mark 16 "Installing the program that files papers."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Papers. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl openssl >"$log" 2>&1 || fail "Keel could not install Papers. Try Open again."
fi
if [ -f "$cfg/auth.secret" ]; then
  secret=$(cat "$cfg/auth.secret")
else
  secret=$(openssl rand -hex 48)
  umask 077
  printf '%s\n' "$secret" > "$cfg/auth.secret"
  chmod 0600 "$cfg/auth.secret"
fi
# Open shows Papers on Keel's own addresses, port 9483 (launch.rs). Papers
# keeps its sign-in only on the address it is told about, so it is told
# about each one: the Keel screen, the house, Away, and the name.
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
away=$(ip -4 addr show tailscale0 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*/\1/p' | head -n 1)
base="http://${house:-127.0.0.1}:9483"
origins="http://127.0.0.1:9483,$base${away:+,http://$away:9483},https://$host"
# The first time, Keel makes the house account itself, so Open signs in.
first=yes
[ -f "$cfg/installed" ] && first=no
image=ghcr.io/papra-hq/papra:26.7.1-root
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 50 "Downloading Papers. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Papers. Check the network and try again."
else
  mark 50 "Opening Papers."
fi
mark 75 "Starting Papers."
podman network exists keel-papers >/dev/null 2>&1 || podman network create keel-papers >"$log" 2>&1 || fail "Keel could not start Papers. Try Open again."
podman rm -f keel-papers >"$log" 2>&1 || true
if ! podman run -d --name keel-papers --hostname papers \
  --network keel-papers --network-alias papers \
  -e APP_BASE_URL="$base" \
  -e TRUSTED_ORIGINS="$origins" \
  -e AUTH_SECRET="$secret" \
  -e DOCUMENT_STORAGE_DRIVER=filesystem \
  -e DOCUMENT_STORAGE_FILESYSTEM_ROOT=/documents \
  -e TZ=UTC \
  -p 127.0.0.1:1221:1221 \
  -v "$cfg/app-data:/app/app-data" \
  -v "$docs:/documents" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on that address. Papers did not start."
  fi
  fail "Keel could not start Papers. Try Open again."
fi
mark 90 "Waiting for the papers page."
up=0
i=0
while [ "$i" -lt 40 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:1221/" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = 1 ] || fail "Papers started, but the page did not answer. Try Open again."
podman network connect keel-papers keel-door >"$log" 2>&1 || true
cat > /etc/systemd/system/keel-papers.service <<'EOF'
[Unit]
Description=Keel papers
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-papers
ExecStop=/usr/bin/podman stop -t 20 keel-papers
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-papers.service >"$log" 2>&1 || true
if [ "$first" = yes ] && [ ! -f /var/lib/keel/signin/papers.json ]; then
  owned=${host#papers.}
  email="papers@$owned"
  account=$(openssl rand -hex 12)
  code=$(curl -sS -o /dev/null --max-time 15 -w '%{http_code}' -X POST "http://127.0.0.1:1221/api/auth/sign-up/email" \
    -H 'Content-Type: application/json' -H "Origin: $base" \
    -d "{\"email\":\"$email\",\"password\":\"$account\",\"name\":\"House\"}" || true)
  case "$code" in
    2*)
      install -d -m 0700 /var/lib/keel/signin
      umask 077
      printf '{"user":"%s","password":"%s"}\n' "$email" "$account" > /var/lib/keel/signin/papers.json
      chmod 0600 /var/lib/keel/signin/papers.json
      ;;
  esac
fi
touch "$cfg/installed"
printf 'ready\n'
exit 0
