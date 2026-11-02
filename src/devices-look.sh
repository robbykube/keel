#!/bin/bash
# Who answered on this house network. The range is this computer's own address, never one someone typed.
set -uo pipefail
fail() { printf '%s\n' "$1" >&2; exit 1; }
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
printf '%s\n' "$house" | awk -F. 'NF==4 && ($1==10 || ($1==192 && $2==168) || ($1==172 && $2+0>=16 && $2+0<=31)) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network."
prefix=${house%.*}
printf 'house|%s\n' "$house"
for i in $(seq 1 254); do
  ping -c 1 -W 1 "$prefix.$i" >/dev/null 2>&1 &
done
wait
ip -4 neigh show | while IFS= read -r line; do
  ip=${line%% *}
  case "$ip" in
    "$prefix".*) ;;
    *) continue ;;
  esac
  printf '%s\n' "$line" | grep -q lladdr || continue
  printf '%s\n' "$line" | grep -Eq 'FAILED|INCOMPLETE' && continue
  mac=$(printf '%s\n' "$line" | sed -n 's/.*lladdr \([^ ]*\).*/\1/p')
  [ -n "$mac" ] || continue
  printf '%s|%s|\n' "$ip" "$mac"
done
