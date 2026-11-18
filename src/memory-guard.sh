set -uo pipefail
# When memory runs short, this computer should slow down or restart the screen,
# not lock up. Keel runs this at every start; it only writes what changed.
changed=0
put() {
  # put <path> <mode>; the content comes on stdin.
  tmp="$1.keel-new"
  install -d -m 0755 "$(dirname "$1")"
  cat > "$tmp"
  if [ -f "$1" ] && cmp -s "$tmp" "$1"; then
    rm -f "$tmp"
    return 0
  fi
  chmod "$2" "$tmp"
  mv "$tmp" "$1"
  changed=1
}

# Compressed swap in memory. Pages the screen is not using are squeezed
# instead of the computer stopping.
put /usr/lib/keel/zram-swap 0755 <<'EOF'
#!/bin/sh
set -u
if [ "${1:-on}" = "off" ]; then
  for dev in $(awk '$1 ~ "^/dev/zram" { print $1 }' /proc/swaps); do
    swapoff "$dev" 2>/dev/null || true
    zramctl --reset "$dev" 2>/dev/null || true
  done
  exit 0
fi
grep -q '^/dev/zram' /proc/swaps && exit 0
modprobe zram 2>/dev/null || true
mem_kb=$(awk '/^MemTotal:/ { print $2 }' /proc/meminfo)
size_kb=$((mem_kb / 2))
[ "$size_kb" -gt 4194304 ] && size_kb=4194304
dev=$(zramctl --find --size "${size_kb}KiB" --algorithm zstd 2>/dev/null) \
  || dev=$(zramctl --find --size "${size_kb}KiB" 2>/dev/null) \
  || { echo "This computer has no compressed memory." >&2; exit 0; }
mkswap -L keel-zram "$dev" >/dev/null 2>&1 || { zramctl --reset "$dev"; exit 0; }
swapon --priority 100 "$dev" 2>/dev/null || zramctl --reset "$dev"
exit 0
EOF

put /etc/systemd/system/keel-zram.service 0644 <<'EOF'
[Unit]
Description=Keel compressed memory
DefaultDependencies=no
Before=swap.target keel.service keel-kiosk.service
Conflicts=shutdown.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/lib/keel/zram-swap on
ExecStop=/usr/lib/keel/zram-swap off

[Install]
WantedBy=swap.target
EOF

put /etc/sysctl.d/90-keel-memory.conf 0644 <<'EOF'
# Compressed memory is fast, so use it before dropping files from memory.
vm.swappiness=100
vm.page-cluster=0
EOF

# The screen may use part of the memory. Past that it slows, then it alone is
# stopped and starts again. Keel and the file sharing are kept.
put /etc/systemd/system/keel-kiosk.service.d/memory.conf 0644 <<'EOF'
[Service]
MemoryHigh=40%
MemoryMax=50%
OOMScoreAdjust=600
EOF

put /etc/systemd/system/keel.service.d/memory.conf 0644 <<'EOF'
[Service]
OOMScoreAdjust=-600
EOF

put /etc/systemd/system/smbd.service.d/keel-memory.conf 0644 <<'EOF'
[Service]
OOMScoreAdjust=-300
EOF

# The kept logs live in /run, which is memory. Uncapped they filled it, and
# then no task could start: Video would not open and Logs would not load.
journal_before=$changed
changed=0
put /etc/systemd/journald.conf.d/90-keel.conf 0644 <<'EOF'
[Journal]
RuntimeMaxUse=16M
RuntimeKeepFree=64M
SystemMaxUse=200M
EOF
if [ "$changed" = 1 ]; then
  systemctl restart systemd-journald >/dev/null 2>&1 || true
  journalctl --vacuum-size=16M >/dev/null 2>&1 || true
fi
[ "$journal_before" = 1 ] && changed=1

[ "$changed" = 1 ] && systemctl daemon-reload >/dev/null 2>&1
sysctl -q -p /etc/sysctl.d/90-keel-memory.conf >/dev/null 2>&1 || true
systemctl enable keel-zram.service >/dev/null 2>&1 || true
systemctl start keel-zram.service >/dev/null 2>&1 || true

# Apply to what is already running, without restarting it.
if systemctl cat keel-kiosk.service >/dev/null 2>&1; then
  systemctl set-property --runtime keel-kiosk.service MemoryHigh=40% MemoryMax=50% >/dev/null 2>&1 || true
  for pid in $(systemctl show -p ControlGroup --value keel-kiosk.service 2>/dev/null | xargs -r -I{} cat /sys/fs/cgroup{}/cgroup.procs 2>/dev/null); do
    choom -p "$pid" -n 600 >/dev/null 2>&1 || true
  done
fi
keel_pid=$(systemctl show -p MainPID --value keel.service 2>/dev/null)
[ -n "$keel_pid" ] && [ "$keel_pid" != 0 ] && choom -p "$keel_pid" -n -600 >/dev/null 2>&1
smbd_pid=$(systemctl show -p MainPID --value smbd.service 2>/dev/null)
[ -n "$smbd_pid" ] && [ "$smbd_pid" != 0 ] && choom -p "$smbd_pid" -n -300 >/dev/null 2>&1
exit 0
