set -uo pipefail
cfg=/var/lib/keel/caddy
log=/var/lib/keel/door-setup.log
ver=2.11.9
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/door.json.tmp
  mv /run/keel/door.json.tmp /run/keel/door.json
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
ready() {
  page_ok "http://127.0.0.1/" && page_ok "http://$house/"
}
make_room() {
  avail=$(df -Pk /run 2>/dev/null | awk 'NR==2 { print $4 }')
  case "$avail" in
    ''|*[!0-9]*) return 0 ;;
  esac
  if [ "$avail" -ge 32768 ]; then
    return 0
  fi
  journalctl --vacuum-size=8M >/dev/null 2>&1 || true
  rm -rf /run/log/journal /run/log/journal.* >/dev/null 2>&1 || true
}
write_helper() {
  cat > /var/lib/keel/caddy-reload.sh <<'EOF'
#!/bin/bash
set -uo pipefail
cfg=/var/lib/keel/caddy
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || exit 1
install -d -m 0755 "$cfg/sites" "$cfg/data"
{
  printf '%s\n' '{' '	admin 127.0.0.1:2019' '	auto_https disable_redirects' '	storage file_system {' "		root $cfg/data" '	}' '}'
  printf '%s\n' 'http://127.0.0.1 {' '	bind 127.0.0.1' '	respond "Keel" 200' '}'
  printf '%s\n' "http://$house {" "	bind $house" '	respond "Keel" 200' '}'
  for file in "$cfg/sites"/*; do
    [ -f "$file" ] || continue
    host=$(basename "$file")
    printf '%s\n' "$host" | grep -Eq '^[a-z0-9.-]+$' || continue
    read -r upstream email < "$file" || continue
    printf '%s\n' "$upstream" | grep -Eq '^127\.0\.0\.1:[0-9]+$' || continue
    printf '%s\n' "http://$host {" '	bind 127.0.0.1 '"$house" "	reverse_proxy $upstream" '}'
    printf '%s\n' "$host {" "	bind $house" "	reverse_proxy $upstream" '	tls {' '		issuer acme {' "			email $email" '		}' '	}' '}'
  done
} > "$cfg/Caddyfile"
if systemctl is-active --quiet keel-door && systemctl show -p ExecStart keel-door | grep -q /usr/local/bin/caddy; then
  /usr/local/bin/caddy reload --config "$cfg/Caddyfile" >/var/lib/keel/door.log 2>&1 || exit 1
fi
exit 0
EOF
  chmod 755 /var/lib/keel/caddy-reload.sh
}
install_unit() {
  cat > /etc/systemd/system/keel-door.service <<'EOF'
[Unit]
Description=Keel front door
After=network-online.target
Wants=network-online.target
[Service]
Type=simple
ExecStart=/usr/local/bin/caddy run --config /var/lib/keel/caddy/Caddyfile
ExecReload=/usr/local/bin/caddy reload --config /var/lib/keel/caddy/Caddyfile
Restart=on-failure
RestartSec=1
[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload >"$log" 2>&1 || true
  systemctl enable keel-door.service >"$log" 2>&1 || true
}
mark 8 "Opening the front door."
install -d -m 0755 "$cfg/sites" "$cfg/data" /var/lib/keel /run/keel
if ! command -v curl >/dev/null 2>&1; then
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true curl ca-certificates >"$log" 2>&1 || fail "Keel could not install the front door. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so the front door did not start."
if ! /usr/local/bin/caddy version 2>/dev/null | grep -q "v$ver"; then
  mark 40 "Downloading the front door."
  case "$(uname -m)" in
    aarch64|arm64)
      file="caddy_${ver}_linux_arm64.tar.gz"
      sum=9e5e76d7888542924de2c25353ae87c5f77e2a22c5b1198633cae13688922282e05383d142599205c88d8d80eee5e967d291214d7abce4835329a191b20dd6e0
      ;;
    x86_64)
      file="caddy_${ver}_linux_amd64.tar.gz"
      sum=0e85544340290a16ce4bfb76fe7d65e9de1bf3a6e67f5ccfff30b30d27ea4c73e078077b7bf3e58ae60b6b12e44327705287059c775bca5782e8ee85ea7edd75
      ;;
    *) fail "This computer cannot run the front door." ;;
  esac
  tmp=$(mktemp -d)
  curl -fsSL -o "$tmp/$file" "https://github.com/caddyserver/caddy/releases/download/v$ver/$file" >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
  printf '%s  %s\n' "$sum" "$tmp/$file" | sha512sum -c - >"$log" 2>&1 || fail "Keel could not trust the front door download. Try Open again."
  tar -xzf "$tmp/$file" -C "$tmp" caddy >"$log" 2>&1 || fail "Keel could not install the front door. Try Open again."
  install -m 0755 "$tmp/caddy" /usr/local/bin/caddy
  rm -rf "$tmp"
fi
mark 70 "Starting the front door."
make_room
write_helper
systemctl disable --now caddy.service >/dev/null 2>&1 || true
systemctl stop keel-door-page >/dev/null 2>&1 || true
systemctl reset-failed keel-door-page >/dev/null 2>&1 || true
systemctl stop keel-door.service >/dev/null 2>&1 || true
systemctl reset-failed keel-door.service >/dev/null 2>&1 || true
if command -v podman >/dev/null 2>&1; then
  podman rm -f keel-door >"$log" 2>&1 || true
fi
rm -f /var/lib/keel/door-forward.py /var/lib/keel/door-publish.sh
/var/lib/keel/caddy-reload.sh >"$log" 2>&1 || fail "Keel could not write the front door. Try Open again."
install_unit
systemctl reset-failed keel-door.service >/dev/null 2>&1 || true
if ! systemctl restart keel-door.service >"$log" 2>&1; then
  last=$(sed '/^$/d' "$log" 2>/dev/null | tail -n 1 | cut -c 1-140)
  fail "${last:-Keel could not start the front door. Try Open again.}"
fi
i=0
while [ "$i" -lt 20 ]; do
  ready && break
  i=$((i + 1))
  sleep 1
done
ready || fail "The front door started, but the page did not answer. Try Open again."
touch "$cfg/installed"
printf '%s\n' "$house"
exit 0
