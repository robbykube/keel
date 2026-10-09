#!/bin/sh
set -eu

# The appliance screen: one browser, full-screen, no desktop around it and
# nothing to exit into.

KIOSK_URL="${KEEL_KIOSK_URL:-http://127.0.0.1:9478/}"
# Under /run, so systemd's RuntimeDirectory clears it with the service. A
# persistent profile caches the GUI and would survive an update.
PROFILE_DIR="${KEEL_KIOSK_PROFILE:-/run/keel-kiosk/profile}"

# No KMS device means no display this boot: a serial console, a headless server,
# or a GPU with no driver in the image. Fall back to the text banner rather than
# leaving a black screen and a spinning restart loop.
# keel-console.service already paints the banner on tty1, and with getty masked
# nothing overwrites it. Just stand down.
if ! ls /dev/dri/card* >/dev/null 2>&1; then
  echo "no KMS display device; leaving tty1 to the text banner" >&2
  # 64 is RestartPreventExitStatus in keel-kiosk.service. A machine does not
  # grow a GPU while running, so retrying every two seconds forever is noise.
  exit 64
fi

# Wait for keel to answer before painting anything. Otherwise the first frame a
# new owner ever sees is a browser connection error.
attempt=0
while [ "${attempt}" -lt 60 ]; do
  if curl -fsS -o /dev/null "http://127.0.0.1:9478/api/health" 2>/dev/null; then
    break
  fi
  attempt=$((attempt + 1))
  sleep 1
done

mkdir -p "${PROFILE_DIR}"

# A NAS is routinely plugged into a monitor with no keyboard or mouse attached.
# wlroots' libinput backend aborts when it enumerates zero input devices, which
# would take the screen down precisely in the case this appliance is built for.
export WLR_LIBINPUT_NO_DEVICES=1
export XDG_SESSION_TYPE=wayland

# pam_systemd should have created this via PAMName. It is a long-running upstream
# complaint that it sometimes has not (cage-kiosk/cage#336), and cage exits hard
# when it is missing, so do not depend on it.
if [ -z "${XDG_RUNTIME_DIR:-}" ]; then
  XDG_RUNTIME_DIR="/run/keel-kiosk"
  export XDG_RUNTIME_DIR
  mkdir -p "${XDG_RUNTIME_DIR}"
  chmod 0700 "${XDG_RUNTIME_DIR}"
  echo "XDG_RUNTIME_DIR was unset; falling back to ${XDG_RUNTIME_DIR}" >&2
fi

# -s allows VT switching, which is what keeps the hidden Ctrl+Alt+F3 recovery
# console reachable from inside the kiosk. cage exits when the browser exits, so
# systemd restarts a whole clean screen rather than a bare compositor.
exec cage -s -- \
  chromium \
    --kiosk \
    --ozone-platform=wayland \
    --user-data-dir="${PROFILE_DIR}" \
    --no-first-run \
    --noerrdialogs \
    --disable-infobars \
    --disable-session-crashed-bubble \
    --disable-pinch \
    --overscroll-history-navigation=0 \
    --password-store=basic \
    --autoplay-policy=no-user-gesture-required \
    "${KIOSK_URL}"
