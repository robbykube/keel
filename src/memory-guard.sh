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
Storage=persistent
SystemMaxUse=64M
SystemKeepFree=1G
RuntimeMaxUse=16M
RuntimeKeepFree=64M
EOF
# Logs kept on the disk survive a freeze, so the next start can show what
# happened just before. They are capped small for the SD card.
if [ "$changed" = 1 ]; then
  install -d -m 2755 -g systemd-journal /var/log/journal 2>/dev/null || install -d -m 2755 /var/log/journal
  systemctl restart systemd-journald >/dev/null 2>&1 || true
  journalctl --flush >/dev/null 2>&1 || true
  journalctl --vacuum-size=64M >/dev/null 2>&1 || true
fi
[ "$journal_before" = 1 ] && changed=1

# Apps (every container) share one part of memory, so Photos or Video
# slows down or stops on its own instead of locking up the whole computer.
put /etc/systemd/system/machine.slice.d/90-keel.conf 0644 <<'EOF'
[Slice]
MemoryHigh=55%
MemoryMax=65%
EOF

# When memory stays jammed for three minutes, stop the largest app. Restart
# the computer only if that did not help. A slow minute is left alone.
put /usr/lib/keel/memory-watch 0755 <<'EOF'
#!/bin/bash
set -u
pressure=/proc/pressure/memory
[ -r "$pressure" ] || exit 0
note=/var/lib/keel/memory-watch.log
jam_for=6        # checks of 30 seconds: three minutes
limit=40         # % of the last minute everything waited for memory
jammed=0
stopped_at=0
app_of() {
  case "$1" in
    keel-photos*) echo photos ;;
    keel-jitsi*) echo jitsi ;;
    keel-jellyfin*) echo jellyfin ;;
    keel-passwords*) echo passwords ;;
    keel-papers*) echo papers ;;
    keel-mail*) echo mail ;;
    *) echo "" ;;   # Names and the front door are never stopped.
  esac
}
title_of() {
  case "$1" in
    photos) echo Photos ;; jitsi) echo Video ;; jellyfin) echo Jellyfin ;;
    passwords) echo Passwords ;; papers) echo Papers ;; mail) echo Mail ;;
  esac
}
largest_app() {
  declare -A used=()
  while read -r id name; do
    app=$(app_of "$name")
    [ -n "$app" ] || continue
    mem=$(cat "/sys/fs/cgroup/machine.slice/libpod-$id.scope/memory.current" 2>/dev/null || echo 0)
    used[$app]=$(( ${used[$app]:-0} + mem ))
  done < <(timeout 60 podman ps --no-trunc --format '{{.ID}} {{.Names}}' 2>/dev/null)
  best=""; most=0
  for app in "${!used[@]}"; do
    if [ "${used[$app]}" -gt "$most" ]; then best=$app; most=${used[$app]}; fi
  done
  echo "$best"
}
while sleep 30; do
  full=$(sed -n 's/^full .*avg60=\([0-9]*\).*/\1/p' "$pressure")
  if [ "${full:-0}" -ge "$limit" ]; then jammed=$((jammed + 1)); else jammed=0; fi
  [ "$jammed" -ge "$jam_for" ] || continue
  jammed=0
  now=$(date +%s)
  if [ "$stopped_at" -gt 0 ] && [ $((now - stopped_at)) -lt 600 ]; then
    up=$(cut -d. -f1 /proc/uptime)
    if [ "$up" -ge 900 ]; then
      echo "$(date -Is) restart Memory stayed full after an app was stopped." >> "$note"
      echo "keel: memory stayed full after stopping an app; restarting the computer"
      systemctl reboot
      exit 0
    fi
    continue
  fi
  app=$(largest_app)
  [ -n "$app" ] || continue
  echo "keel: memory was full for three minutes; stopping $(title_of "$app")"
  for name in $(timeout 60 podman ps --format '{{.Names}}' 2>/dev/null); do
    [ "$(app_of "$name")" = "$app" ] && timeout 60 podman stop -t 10 "$name" >/dev/null 2>&1
  done
  rm -f "/run/keel/$app.json"
  echo "$(date -Is) stopped $(title_of "$app")" >> "$note"
  stopped_at=$now
done
EOF

put /etc/systemd/system/keel-memory-watch.service 0644 <<'EOF'
[Unit]
Description=Keel stops the largest app when memory stays full
After=keel.service

[Service]
ExecStart=/usr/lib/keel/memory-watch
Restart=always
RestartSec=30s
OOMScoreAdjust=-900
Nice=-5

[Install]
WantedBy=multi-user.target
EOF

# The screen's browser keeps its profile in /run, which is memory. Its
# cache of pictures seen on the screen filled /run and then nothing could
# start. 32 MB is plenty for pages served by this computer.
screen_before=$changed
changed=0
put /etc/chromium/policies/managed/keel.json 0644 <<'EOF'
{ "DiskCacheSize": 33554432 }
EOF
if [ "$changed" = 1 ] && systemctl is-active --quiet keel-kiosk.service 2>/dev/null; then
  rm -rf /run/keel-kiosk/profile*/Default/Cache /run/keel-kiosk/profile*/Default/"Code Cache" 2>/dev/null || true
  systemctl restart keel-kiosk.service >/dev/null 2>&1 || true
fi
[ "$screen_before" = 1 ] && changed=1

# A quicker start. Keel does not need the network to start, and copes when
# it comes later, so the start waits for it 10 seconds at most instead of
# two minutes (an unplugged cable, or a router still starting after a power
# cut, left the screen black that long).
start_before=$changed
changed=0
wait_online=$(command -v systemd-networkd-wait-online 2>/dev/null || echo /usr/lib/systemd/systemd-networkd-wait-online)
if [ -x "$wait_online" ]; then
  put /etc/systemd/system/systemd-networkd-wait-online.service.d/90-keel.conf 0644 <<EOF
[Service]
ExecStart=
ExecStart=$wait_online --any --timeout=10
EOF
fi
[ "$changed" = 1 ] && systemctl daemon-reload >/dev/null 2>&1
[ "$start_before" = 1 ] && changed=1

# Programs Keel does not use stop starting with the computer: printing,
# Bluetooth, finding printers on the network, modems, cron and sensors.
# Wi-Fi only when no Wi-Fi connection is in use. They stay installed.
for unit in cups.service cups-browsed.service cups.socket bluetooth.service \
    avahi-daemon.service avahi-daemon.socket ModemManager.service cron.service \
    lm-sensors.service; do
  if systemctl is-enabled "$unit" >/dev/null 2>&1; then
    systemctl disable --now "$unit" >/dev/null 2>&1 || true
  fi
done
wifi_used=0
for dev in /sys/class/net/*; do
  [ -d "$dev/wireless" ] || continue
  [ "$(cat "$dev/operstate" 2>/dev/null)" = up ] && wifi_used=1
done
if [ "$wifi_used" = 0 ] && systemctl is-enabled wpa_supplicant.service >/dev/null 2>&1; then
  systemctl disable --now wpa_supplicant.service >/dev/null 2>&1 || true
fi

# A computer that stops answering completely restarts by itself. The chip
# waits 15 seconds without a sign of life (the most a Pi 4 can wait). A slow
# or busy computer still gives that sign, so it is left alone.
watchdog_before=$changed
changed=0
if [ -e /dev/watchdog0 ] || [ -e /dev/watchdog ]; then
  put /etc/systemd/system.conf.d/90-keel-watchdog.conf 0644 <<'EOF'
[Manager]
RuntimeWatchdogSec=15s
RebootWatchdogSec=2min
EOF
fi
[ "$changed" = 1 ] && systemctl daemon-reexec >/dev/null 2>&1
[ "$watchdog_before" = 1 ] && changed=1

# A slow start (a disk check after the power was pulled) shows on the
# screen instead of a black screen. Quick starts stay quiet.
cmdline=/boot/firmware/cmdline.txt
if [ -w "$cmdline" ] && grep -q 'systemd\.show_status=false' "$cmdline"; then
  sed 's/systemd\.show_status=false/systemd.show_status=auto/' "$cmdline" > "$cmdline.keel-new" \
    && grep -q 'root=' "$cmdline.keel-new" \
    && mv "$cmdline.keel-new" "$cmdline"
  rm -f "$cmdline.keel-new"
fi

[ "$changed" = 1 ] && systemctl daemon-reload >/dev/null 2>&1
sysctl -q -p /etc/sysctl.d/90-keel-memory.conf >/dev/null 2>&1 || true
systemctl enable keel-zram.service >/dev/null 2>&1 || true
systemctl start keel-zram.service >/dev/null 2>&1 || true
systemctl enable keel-memory-watch.service >/dev/null 2>&1 || true
systemctl restart keel-memory-watch.service >/dev/null 2>&1 || true
systemctl set-property --runtime machine.slice MemoryHigh=55% MemoryMax=65% >/dev/null 2>&1 || true

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
