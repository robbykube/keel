#!/bin/bash
# Keyboards and headphones. The address is checked before it is used.
set -uo pipefail
action="${1:-status}"
mac="${2:-}"
job=/var/lib/keel/bluetooth-job
answer=/var/lib/keel/bluetooth-answer
fail() { printf 'error|%s\n' "$1" >&2; exit 1; }
case "$mac" in
  ""|[0-9A-Fa-f][0-9A-Fa-f]:[0-9A-Fa-f][0-9A-Fa-f]:[0-9A-Fa-f][0-9A-Fa-f]:[0-9A-Fa-f][0-9A-Fa-f]:[0-9A-Fa-f][0-9A-Fa-f]:[0-9A-Fa-f][0-9A-Fa-f]) ;;
  *) fail "That is not a Bluetooth device." ;;
esac
command -v bluetoothctl >/dev/null 2>&1 || fail "Bluetooth is still starting. Try again in a minute."
mkdir -p /var/lib/keel
note() { printf '%s\n' "$1" > "$job"; }
rfkill unblock bluetooth >/dev/null 2>&1 || true

bring_up() {
  bluetoothctl power on >/dev/null 2>&1 || true
  bluetoothctl list 2>/dev/null | grep -q '^Controller ' && return 0
  modprobe hci_uart >/dev/null 2>&1 || true
  modprobe btbcm >/dev/null 2>&1 || true
  console=$(cat /sys/class/tty/console/active 2>/dev/null || true)
  if [ -e /dev/ttyAMA0 ] && ! printf '%s\n' "$console" | grep -q ttyAMA0 && ! pgrep -x btattach >/dev/null 2>&1; then
    btattach -B /dev/ttyAMA0 -P bcm -S 3000000 >/dev/null 2>&1 &
    sleep 2
  fi
  bluetoothctl power on >/dev/null 2>&1 || true
  bluetoothctl list 2>/dev/null | grep -q '^Controller ' && return 0
  fail "Bluetooth did not start on this computer."
}

clean_name() {
  printf '%s\n' "$1" | tr '|' ' ' | tr -d '\r' | head -n 1
}

one() {
  local addr="$1" name paired connected info
  info=$(bluetoothctl info "$addr" 2>/dev/null || true)
  name=$(printf '%s\n' "$info" | sed -n 's/^[[:space:]]*Name: //p' | head -n 1)
  [ -n "$name" ] || name=$(printf '%s\n' "$info" | sed -n 's/^[[:space:]]*Alias: //p' | head -n 1)
  [ -n "$name" ] || name="$2"
  paired=no
  connected=no
  printf '%s\n' "$info" | grep -q 'Paired: yes' && paired=yes
  printf '%s\n' "$info" | grep -q 'Connected: yes' && connected=yes
  printf 'device|%s|%s|%s|%s\n' "$addr" "$(clean_name "$name")" "$paired" "$connected"
}

list_devices() {
  local line addr name
  bluetoothctl devices 2>/dev/null | while IFS= read -r line; do
    case "$line" in
      Device\ *) ;;
      *) continue ;;
    esac
    addr=$(printf '%s\n' "$line" | awk '{ print $2 }')
    name=$(printf '%s\n' "$line" | cut -d' ' -f3-)
    one "$addr" "$name"
  done
}

bring_up

case "$action" in
  status)
    printf 'message|Press Look to find a keyboard or headphones.\n'
    list_devices
    ;;
  scan)
    note "state|looking"
    bluetoothctl --timeout 12 scan on >/dev/null 2>&1 || true
    printf 'message|Press Pair on the keyboard or headphones. Put it in pairing mode first.\n'
    list_devices
    note "state|idle"
    ;;
  connect)
    [ -n "$mac" ] || fail "Choose a device first."
    out=$(bluetoothctl connect "$mac" 2>&1 || true)
    printf '%s\n' "$out" | grep -Eqi 'Connection successful|Connected: yes' || fail "Could not connect. Press Pair again."
    printf 'message|Connected.\n'
    list_devices
    ;;
  disconnect)
    [ -n "$mac" ] || fail "Choose a device first."
    bluetoothctl disconnect "$mac" >/dev/null 2>&1 || true
    printf 'message|Disconnected.\n'
    list_devices
    ;;
  forget)
    [ -n "$mac" ] || fail "Choose a device first."
    bluetoothctl disconnect "$mac" >/dev/null 2>&1 || true
    bluetoothctl remove "$mac" >/dev/null 2>&1 || fail "Keel could not forget that device."
    printf 'message|Forgotten. Pair it again when you want it.\n'
    list_devices
    ;;
  pair)
    [ -n "$mac" ] || fail "Choose a device first."
    rm -f "$answer"
    note $'state|pairing\nmac|'"$mac"
    coproc BT { stdbuf -oL -eL bluetoothctl 2>&1; }
    sleep 0.4
    printf '%s\n' "power on" "agent KeyboardDisplay" "default-agent" "pair $mac" >&"${BT[1]}"
    buf=""
    end=$((SECONDS + 90))
    asked=""
    while [ "$SECONDS" -lt "$end" ]; do
      chunk=""
      read -t 1 -r -n 400 chunk <&"${BT[0]}" || true
      buf="$buf$chunk"
      if [ -z "$asked" ] && printf '%s\n' "$buf" | grep -q 'Confirm passkey'; then
        code=$(printf '%s\n' "$buf" | sed -n 's/.*[Pp]asskey[^0-9]*\([0-9][0-9][0-9][0-9][0-9][0-9]\).*/\1/p' | tail -n 1)
        note "state|confirm"$'\n'"code|$code"$'\n'"mac|$mac"
        asked=yes
        rm -f "$answer"
        while [ ! -s "$answer" ] && [ "$SECONDS" -lt "$end" ]; do sleep 0.3; done
        ans=$(head -n 1 "$answer" 2>/dev/null || printf 'no')
        printf '%s\n' "$ans" >&"${BT[1]}"
      elif [ -z "$asked" ] && printf '%s\n' "$buf" | grep -q 'Enter PIN'; then
        note "state|pin"$'\n'"mac|$mac"
        asked=pin
        rm -f "$answer"
        while [ ! -s "$answer" ] && [ "$SECONDS" -lt "$end" ]; do sleep 0.3; done
        ans=$(head -n 1 "$answer" 2>/dev/null || true)
        printf '%s\n' "$ans" >&"${BT[1]}"
      elif [ -z "$asked" ] && printf '%s\n' "$buf" | grep -Eq 'Passkey:|PIN code:'; then
        code=$(printf '%s\n' "$buf" | sed -n 's/.*[^0-9]\([0-9][0-9][0-9][0-9][0-9][0-9]\).*/\1/p' | tail -n 1)
        note "state|keyboard"$'\n'"code|$code"$'\n'"mac|$mac"
        asked=keyboard
      fi
      if printf '%s\n' "$buf" | grep -Eq 'Paired: yes|Pairing successful'; then
        printf '%s\n' "trust $mac" "connect $mac" >&"${BT[1]}"
        sleep 2
        break
      fi
      if printf '%s\n' "$buf" | grep -Eq 'Failed to pair|AuthenticationFailed|org.bluez.Error'; then
        printf 'quit\n' >&"${BT[1]}" || true
        note "state|idle"
        fail "Pairing did not work. Put the device in pairing mode and press Pair again."
      fi
    done
    printf 'quit\n' >&"${BT[1]}" || true
    wait "$BT_PID" 2>/dev/null || true
    note "state|idle"
    if bluetoothctl info "$mac" 2>/dev/null | grep -q 'Paired: yes'; then
      printf 'message|Paired.\n'
      list_devices
    else
      fail "Pairing stopped. Put the device in pairing mode and press Pair again."
    fi
    ;;
  *)
    fail "Keel cannot do that."
    ;;
esac
