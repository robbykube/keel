set -uo pipefail
cfg=/var/lib/npm
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
keep_log() {
  install -d -m 0755 /var/lib/keel
  program=$(podman inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' keel-door 2>/dev/null | awk '{print $1}')
  direct=none
  if printf '%s\n' "$program" | awk -F. 'NF==4 { ok=1 } END { exit ok?0:1 }'; then
    direct=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://$program:81/" || true)
  fi
  {
    printf '%s\n' "---- $(date -Is) ----"
    printf 'program %s direct %s localhost %s house %s\n' \
      "${program:-none}" "$direct" \
      "$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:81/" || true)" \
      "$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://${house:-}:81/" || true)"
    systemctl is-active keel-door-page 2>/dev/null || true
    podman logs --tail 80 keel-door 2>&1 || true
  } >> /var/lib/keel/door.log 2>/dev/null || true
}
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
  page_ok "http://127.0.0.1:81/" && page_ok "http://$house:81/"
}
finish() {
  cat > /etc/systemd/system/keel-door.service <<'EOF'
[Unit]
Description=Keel front door
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStartPre=-/bin/sh -c 'sysctl -w net.ipv4.ip_forward=1; sysctl -w net.ipv4.conf.all.route_localnet=1; iptables -P FORWARD ACCEPT; iptables -C FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT || iptables -I FORWARD 1 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT; iptables -C FORWARD -p tcp -m multiport --dports 80,81,443 -j ACCEPT || iptables -I FORWARD 1 -p tcp -m multiport --dports 80,81,443 -j ACCEPT'
ExecStart=/usr/bin/podman start keel-door
ExecStartPost=-/var/lib/keel/door-publish.sh
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
# The page is inside the program. These let this computer pass a visitor through to it.
install -d -m 0755 /etc/sysctl.d
printf '%s\n' 'net.ipv4.ip_forward=1' 'net.ipv4.conf.all.route_localnet=1' > /etc/sysctl.d/keel-forward.conf
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
iptables -P FORWARD ACCEPT >/dev/null 2>&1 || true
iptables -C FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT >/dev/null 2>&1 || iptables -I FORWARD 1 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT >/dev/null 2>&1 || true
iptables -C FORWARD -p tcp -m multiport --dports 80,81,443 -j ACCEPT >/dev/null 2>&1 || iptables -I FORWARD 1 -p tcp -m multiport --dports 80,81,443 -j ACCEPT >/dev/null 2>&1 || true
# Old rules sent the page to a copy of the program that is gone. Drop them.
install -d -m 0755 /var/lib/keel
cat > /var/lib/keel/door-forward.py <<'PY'
#!/usr/bin/env python3
import socket, threading, sys
dest = sys.argv[1]
pairs = sys.argv[2:]
def pipe(left, right):
    try:
        while True:
            data = left.recv(16384)
            if not data:
                break
            right.sendall(data)
    except Exception:
        pass
    for item in (left, right):
        try:
            item.shutdown(socket.SHUT_RDWR)
        except Exception:
            pass
def handle(client, dport):
    try:
        remote = socket.create_connection((dest, int(dport)), 5)
    except Exception:
        client.close()
        return
    threading.Thread(target=pipe, args=(client, remote), daemon=True).start()
    pipe(remote, client)
    client.close()
    remote.close()
def serve(bind, port, dport):
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        srv.bind((bind, int(port)))
    except Exception as exc:
        print("could not open %s:%s: %s" % (bind, port, exc), flush=True)
        return
    srv.listen(40)
    while True:
        client, _ = srv.accept()
        threading.Thread(target=handle, args=(client, dport), daemon=True).start()
index = 0
threads = []
while index + 2 < len(pairs):
    thread = threading.Thread(target=serve, args=(pairs[index], pairs[index + 1], pairs[index + 2]), daemon=True)
    thread.start()
    threads.append(thread)
    index += 3
if not threads:
    raise SystemExit(1)
for thread in threads:
    thread.join()
PY
cat > /var/lib/keel/door-publish.sh <<'EOF'
#!/bin/sh
drop_old() {
  chain=$1
  iptables -t nat -S "$chain" 2>/dev/null | while read -r rule; do
    case "$rule" in
      *to-destination\ 10.*|*-d\ 10.*)
        del=$(printf '%s\n' "$rule" | sed 's/^-A /-D /')
        # shellcheck disable=SC2086
        iptables -t nat $del >/dev/null 2>&1 || true
        ;;
    esac
  done
}
drop_old PREROUTING
drop_old OUTPUT
drop_old POSTROUTING
ip=
i=0
while [ "$i" -lt 20 ]; do
  ip=$(podman inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' keel-door 2>/dev/null | awk '{print $1}')
  printf '%s\n' "$ip" | awk -F. 'NF==4 { ok=1 } END { exit ok?0:1 }' && break
  i=$((i + 1))
  sleep 1
done
printf '%s\n' "$ip" | awk -F. 'NF==4 { ok=1 } END { exit ok?0:1 }' || exit 0
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
printf '%s\n' "$house" | awk -F. 'NF==4 { ok=1 } END { exit ok?0:1 }' || exit 0
sub=$(podman network inspect keel-front --format '{{range .Subnets}}{{.Subnet}} {{end}}' 2>/dev/null | awk '{for (n=1;n<=NF;n++) if ($n ~ /^[0-9.]+\/[0-9]+$/) { print $n; exit }}')
if printf '%s\n' "$sub" | awk -F'[./]' 'NF>=5 { ok=1 } END { exit ok?0:1 }'; then
  iptables -t nat -C POSTROUTING -s "$sub" -j MASQUERADE >/dev/null 2>&1 || iptables -t nat -A POSTROUTING -s "$sub" -j MASQUERADE >/dev/null 2>&1 || true
fi
systemctl stop keel-door-page >/dev/null 2>&1 || true
systemctl reset-failed keel-door-page >/dev/null 2>&1 || true
py=$(command -v python3)
[ -n "$py" ] || exit 0
systemd-run --unit=keel-door-page --collect "$py" /var/lib/keel/door-forward.py "$ip" \
  127.0.0.1 80 80 \
  127.0.0.1 81 81 \
  "$house" 80 80 \
  "$house" 81 81 \
  "$house" 443 443 \
  >/var/lib/keel/door-forward.log 2>&1 || true
exit 0
EOF
chmod 755 /var/lib/keel/door-publish.sh /var/lib/keel/door-forward.py
if ! command -v python3 >/dev/null 2>&1; then
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true python3 >"$log" 2>&1 || fail "Keel could not open the front door page. Try Open again."
fi
# A start that never answered still left its files. The next Open would
# keep them, and the page would fail the same way.
rm -f "$cfg/installed"
if podman container exists keel-door >/dev/null 2>&1; then
  keep_log
fi
podman rm -f keel-door >"$log" 2>&1 || true
rm -rf "$cfg/data" "$cfg/letsencrypt"
install -d -m 0755 "$cfg/data" "$cfg/letsencrypt"
podman network rm -f keel-front >"$log" 2>&1 || true
podman network create keel-front >"$log" 2>&1 || fail "Keel could not start the front door. Try Open again."
gw=$(ip -4 route show default 2>/dev/null | awk '{print $3; exit}')
dns="--dns 1.1.1.1"
printf '%s\n' "$gw" | awk -F. 'NF==4 && $1!~/^(0|127)$/ { ok=1 } END { exit ok?0:1 }' && dns="$dns --dns $gw"
# shellcheck disable=SC2086
if ! podman run -d --name keel-door --hostname keel-door --network keel-front \
  $dns \
  -e DISABLE_IPV6=true \
  -e TZ=UTC \
  -v "$cfg/data:/data" \
  -v "$cfg/letsencrypt:/etc/letsencrypt" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on port 80 or 443. The front door did not start."
  fi
  last=$(sed '/^$/d' "$log" | tail -n 1 | cut -c 1-140)
  fail "${last:-Keel could not start the front door. Try Open again.}"
fi
/var/lib/keel/door-publish.sh >"$log" 2>&1 || true
mark 90 "Waiting for the front door."
i=0
while [ "$i" -lt 60 ]; do
  ready && finish
  i=$((i + 1))
  sleep 2
done
if ! podman inspect -f '{{.State.Running}}' keel-door 2>/dev/null | grep -qx true; then
  last=$(podman logs --tail 20 keel-door 2>&1 | sed '/^$/d' | tail -n 1 | cut -c 1-160)
  fail "${last:-The front door stopped before the page was ready. Try Open again.}"
fi
keep_log
last=$(podman logs --tail 80 keel-door 2>&1 | sed '/^$/d' | grep -Ei 'emerg|address family|bind\(\)|failed' | tail -n 1 | cut -c 1-140)
fail "The front door started, but the page did not answer. ${last:-Try Open again.}"
