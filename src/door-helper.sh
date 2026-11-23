#!/bin/bash
set -uo pipefail
cfg=/var/lib/keel/caddy
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || exit 1
install -d -m 0755 "$cfg/sites" "$cfg/data" "$cfg/ports"
# Away's address, when this computer is on Away, so an app's port answers there too.
away=$(ip -4 addr show tailscale0 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*/\1/p' | head -n 1)
printf '%s\n' "$away" | grep -Eq '^100\.([6-9][0-9]|1[01][0-9]|12[0-7])\.[0-9]+\.[0-9]+$' || away=""
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
  # Open: each app on its own port of Keel's addresses (launch.rs). Keel's
  # own sign-in cookie is not passed to the app, and /keel-open/ is the
  # one-time sign-in hand-off on Keel's local listener.
  for file in "$cfg/ports"/*; do
    [ -f "$file" ] || continue
    port=$(basename "$file")
    printf '%s\n' "$port" | grep -Eq '^948[1-9]$' || continue
    read -r upstream extra gate < "$file" || continue
    printf '%s\n' "$upstream" | grep -Eq '^127\.0\.0\.1:[0-9]+$' || continue
    # Mail asks the browser for a password; its port lets in only someone
    # Open sent, and Keel hands Mail the password (/keel-gate/).
    ask=""
    if [ "${extra:-}" = gate ] && printf '%s\n' "${gate:-}" | grep -Eq '^[a-z]+$'; then
      ask=$(printf '%s\n' '	@gated not path /keel-open/*' '	forward_auth @gated 127.0.0.1:9477 {' "		uri /keel-gate/$gate" '		copy_headers Authorization' '	}')
    fi
    printf '%s\n' "http://:$port {" "	bind 127.0.0.1 $house${away:+ $away}" '	reverse_proxy /keel-open/* 127.0.0.1:9477'
    [ -n "$ask" ] && printf '%s\n' "$ask"
    printf '%s\n' "	reverse_proxy $upstream {" '		header_up Cookie "keel_session=[^;]*;? ?" ""' '		header_down -X-Frame-Options' '	}' '}'
  done
} > "$cfg/Caddyfile"
if systemctl is-active --quiet keel-door && systemctl show -p ExecStart keel-door | grep -q /usr/local/bin/caddy; then
  /usr/local/bin/caddy reload --config "$cfg/Caddyfile" >/var/lib/keel/door.log 2>&1 || exit 1
fi
exit 0
