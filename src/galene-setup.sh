set -uo pipefail
# Light Video: Galène, one small program from the Keel package, behind the
# front door at video.<name>. Calls use UDP 10000-10019.
name='__KEEL_NAME__'
people='__KEEL_PEOPLE__'
bin=/usr/lib/keel/apps/galene/galene
static=/usr/lib/keel/apps/galene/static
cfg=/var/lib/galene
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/galene.json.tmp
  mv /run/keel/galene.json.tmp /run/keel/galene.json
}
mark 10 "Opening Video."
[ -f "$bin" ] && [ -d "$static" ] || fail "Light Video comes with the next Keel update. Until then, pick the full Video in Settings."
# The package unpacker does not keep the program bit.
chmod 0755 "$bin"
install -d -m 0750 "$cfg" "$cfg/data" "$cfg/groups" "$cfg/recordings"
: > "$log"
# Jitsi and Galène share the meeting address and the call ports.
podman rm -f keel-jitsi-web keel-jitsi-jvb keel-jitsi-jicofo keel-jitsi-prosody >>"$log" 2>&1 || true
rm -f /run/keel/jitsi.json
mark 30 "Writing the meeting settings."
umask 022
cat > "$cfg/data/config.json" <<EOF
{
  "proxyURL": "https://$name/"
}
EOF
# One group, "call": anyone with the link joins with any name, rooms such as
# call/family are made when first opened, and recording is allowed.
cat > "$cfg/groups/call.json" <<EOF
{
  "description": "Calls on this Keel",
  "wildcard-user": { "password": { "type": "wildcard" }, "permissions": "present" },
  "auto-subgroups": true,
  "allow-recording": true,
  "max-clients": $people
}
EOF
cat > /etc/systemd/system/keel-galene.service <<EOF
[Unit]
Description=Keel light Video (Galène)
After=network-online.target

[Service]
ExecStart=$bin -insecure -http 127.0.0.1:8444 -static $static -data $cfg/data -groups $cfg/groups -recordings $cfg/recordings -udp-range 10000-10019 -turn ""
WorkingDirectory=$cfg
Restart=on-failure
RestartSec=3s
LimitNOFILE=65536
MemoryMax=300M
OOMScoreAdjust=300
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >>"$log" 2>&1 || true
systemctl enable keel-galene.service >>"$log" 2>&1 || true
mark 60 "Starting the meeting program."
systemctl restart keel-galene.service >>"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
mark 80 "Waiting for the meeting page."
up=0
i=0
while [ "$i" -lt 30 ]; do
  if curl -fsS -o /dev/null --max-time 3 "http://127.0.0.1:8444/group/call/"; then
    up=1
    break
  fi
  i=$((i + 1))
  sleep 1
done
if [ "$up" != 1 ]; then
  journalctl -u keel-galene -n 40 --no-pager >>"$log" 2>&1 || true
  fail "Video started, but the meeting page did not answer. Logs shows why under Video. Try Open again."
fi
exit 0
