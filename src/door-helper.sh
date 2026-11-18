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
    read -r upstream email extra < "$file" || continue
    printf '%s\n' "$upstream" | grep -Eq '^127\.0\.0\.1:[0-9]+$' || continue
    # Video takes call recordings at /keel-recording/, and only there, on
    # Keel's recording port. Keel's own page is never behind the door.
    rec=""
    [ "${extra:-}" = rec ] && rec='	reverse_proxy /keel-recording/* 127.0.0.1:9477'
    printf '%s\n' "http://$host {" '	bind 127.0.0.1 '"$house" ${rec:+"$rec"} "	reverse_proxy $upstream" '}'
    printf '%s\n' "$host {" "	bind $house" ${rec:+"$rec"} "	reverse_proxy $upstream" '	tls {' '		issuer acme {' "			email $email" '		}' '	}' '}'
  done
} > "$cfg/Caddyfile"
if systemctl is-active --quiet keel-door && systemctl show -p ExecStart keel-door | grep -q /usr/local/bin/caddy; then
  /usr/local/bin/caddy reload --config "$cfg/Caddyfile" >/var/lib/keel/door.log 2>&1 || exit 1
fi
exit 0
