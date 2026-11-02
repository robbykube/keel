#!/bin/sh
# The Pi screen. The small panel and a larger HDMI screen each get the page
# at that screen's own size. Plugging or unplugging a screen starts this again.
set -eu

KIOSK_URL="${KEEL_KIOSK_URL:-http://127.0.0.1:9478/}"

# One name per line. The small panel comes first, then HDMI.
list_connected() {
  small=""
  large=""
  for status in /sys/class/drm/card[0-9]-*/status; do
    [ -f "$status" ] || continue
    [ "$(cat "$status" 2>/dev/null || true)" = "connected" ] || continue
    base=$(basename "$(dirname "$status")")
    card=${base%%-*}
    name=${base#*-}
    driver=""
    if [ -L "/sys/class/drm/${card}/device/driver" ]; then
      driver=$(basename "$(readlink -f "/sys/class/drm/${card}/device/driver")")
    fi
    case "$driver" in
      simpledrm|simple-framebuffer) continue ;;
    esac
    case "$name" in
      DSI-*|DPI-*|LVDS-*) small="${small} ${name}" ;;
      HDMI-*|DP-*|eDP-*|VGA-*) large="${large} ${name}" ;;
    esac
  done
  for name in $small $large; do
    printf '%s\n' "$name"
  done
}

signature() {
  list_connected | sort | tr '\n' ' ' | sed 's/ *$//'
}

browser_flags() {
  printf '%s\n' \
    --kiosk \
    --ozone-platform=wayland \
    --user-data-dir="$1" \
    --no-first-run \
    --noerrdialogs \
    --disable-infobars \
    --disable-session-crashed-bubble \
    --disable-pinch \
    --overscroll-history-navigation=0 \
    --password-store=basic \
    --autoplay-policy=no-user-gesture-required \
    --renderer-process-limit=1 \
    "$KIOSK_URL"
}

prepare_session() {
  export WLR_LIBINPUT_NO_DEVICES=1
  export XDG_SESSION_TYPE=wayland
  if [ -z "${XDG_RUNTIME_DIR:-}" ]; then
    XDG_RUNTIME_DIR="/run/keel-kiosk"
    export XDG_RUNTIME_DIR
    mkdir -p "${XDG_RUNTIME_DIR}"
    chmod 0700 "${XDG_RUNTIME_DIR}"
  fi
}

wait_for_keel() {
  attempt=0
  while [ "${attempt}" -lt 60 ]; do
    if curl -fsS -o /dev/null "http://127.0.0.1:9478/api/health" 2>/dev/null; then
      return 0
    fi
    attempt=$((attempt + 1))
    sleep 1
  done
}

# Called by udev after a screen is plugged or unplugged.
plug() {
  sig=$(signature)
  [ -n "${sig}" ] || exit 0
  prev=""
  if [ -f /run/keel-screens ]; then
    prev=$(cat /run/keel-screens)
  fi
  [ "${sig}" = "${prev}" ] && exit 0
  state=$(systemctl is-active keel-kiosk.service 2>/dev/null || true)
  if [ "${state}" = "activating" ] && [ ! -f /run/keel-screens ]; then
    printf '%s\n' "${sig}" > /run/keel-screens
    exit 0
  fi
  printf '%s\n' "${sig}" > /run/keel-screens
  systemctl restart keel-kiosk.service || true
}

# Started by sway, once per screen, each at that screen's size.
fill() {
  outputs=/run/keel-kiosk/outputs
  if [ ! -s "${outputs}" ]; then
    swaymsg exit || true
    exit 1
  fi
  pids=""
  i=0
  while IFS= read -r name; do
    [ -n "${name}" ] || continue
    i=$((i + 1))
    profile="/run/keel-kiosk/profile-${i}"
    mkdir -p "${profile}"
    # shellcheck disable=SC2046
    chromium $(browser_flags "${profile}") &
    pid=$!
    pids="${pids} ${pid}"
    n=0
    while [ "${n}" -lt 40 ]; do
      if swaymsg -t get_tree 2>/dev/null | grep -Eq "\"pid\": ?${pid}[,}]"; then
        break
      fi
      n=$((n + 1))
      sleep 0.25
    done
    swaymsg "[pid=${pid}] move window to output ${name}, fullscreen enable" || true
  done < "${outputs}"
  if [ -z "${pids}" ]; then
    swaymsg exit || true
    exit 1
  fi
  while :; do
    for pid in ${pids}; do
      if ! kill -0 "${pid}" 2>/dev/null; then
        swaymsg exit || true
        exit 0
      fi
    done
    sleep 2
  done
}

start() {
  if ! ls /dev/dri/card* >/dev/null 2>&1; then
    echo "no KMS display device; leaving tty1 to the text banner" >&2
    exit 64
  fi
  wait_for_keel
  prepare_session
  mkdir -p /run/keel-kiosk
  list_connected > /run/keel-kiosk/outputs || true
  if [ ! -s /run/keel-kiosk/outputs ] || ! command -v sway >/dev/null 2>&1; then
    profile="${KEEL_KIOSK_PROFILE:-/run/keel-kiosk/profile}"
    mkdir -p "${profile}"
    # shellcheck disable=SC2046
    exec cage -s -m extend -- chromium $(browser_flags "${profile}")
  fi
  conf=/run/keel-kiosk/sway.conf
  {
    printf '%s\n' \
      "default_border none" \
      "default_floating_border none" \
      "hide_edge_borders both" \
      "focus_follows_mouse no" \
      "seat seat0 hide_cursor 5000"
    i=1
    while IFS= read -r name; do
      [ -n "${name}" ] || continue
      printf 'workspace %s output %s\n' "${i}" "${name}"
      i=$((i + 1))
    done < /run/keel-kiosk/outputs
    printf '%s\n' "exec /usr/lib/keel/start-kiosk.sh fill"
  } > "${conf}"
  exec sway -c "${conf}"
}

case "${1:-start}" in
  plug) plug ;;
  fill) fill ;;
  start) start ;;
  *) exit 2 ;;
esac
