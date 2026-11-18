set -uo pipefail
owned='__KEEL_OWNED__'
host='__KEEL_HOST__'
forward='__KEEL_FORWARD__'
port='__KEEL_PORT__'
fail() { echo "$1" >&2; exit 1; }
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
urlenc() {
  printf '%s' "$1" | sed 's/%/%25/g; s/ /%20/g; s/+/%2B/g; s/\//%2F/g; s/=/%3D/g; s/@/%40/g'
}
json_token() {
  printf '%s' "$1" | sed -n 's/.*"token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1
}
install -d -m 0755 /run/keel /var/lib/keel/caddy/sites
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so the name was not written."
login=$(curl -sS -X POST "http://127.0.0.1:5380/api/user/login" --data "user=admin&pass=admin" || true)
token=$(json_token "$login")
[ -n "$token" ] || fail "Keel could not sign in to Names, so the name was not written."
etoken=$(urlenc "$token")
curl -sS "http://127.0.0.1:5380/api/zones/create?token=$etoken&zone=$owned&type=Primary" >/dev/null 2>&1 || true
record=$(curl -sS "http://127.0.0.1:5380/api/zones/records/add?token=$etoken&domain=$host&zone=$owned&type=A&ipAddress=$house&ttl=60&overwrite=true" || true)
printf '%s' "$record" | grep -Eq '"status"[[:space:]]*:[[:space:]]*"ok"' || fail "Keel could not write $host."
printf '%s\n' "$host" | grep -Eq '^[a-z0-9.-]+$' || fail "Keel could not send $host to the app."
case "$forward" in
  keel-jitsi-web) upstream=127.0.0.1:8080 ;;
  keel-passwords) upstream=127.0.0.1:8081 ;;
  *) upstream="127.0.0.1:$port" ;;
esac
printf '%s\n' "$upstream" | grep -Eq '^127\.0\.0\.1:[0-9]+$' || fail "Keel could not send $host to the app."
extra=""
case "$forward" in keel-jitsi-web|keel-galene) extra=" rec" ;; esac
printf '%s %s%s\n' "$upstream" "admin@$owned" "$extra" > "/var/lib/keel/caddy/sites/$host"
[ -x /var/lib/keel/caddy-reload.sh ] || fail "The front door is not ready, so the name was not sent."
# The door's helper as this Keel writes it, so an update reaches a door
# that was set up by an older Keel.
cat > /var/lib/keel/caddy-reload.sh.new <<'EOF'
__KEEL_DOOR_HELPER__
EOF
chmod 755 /var/lib/keel/caddy-reload.sh.new && mv /var/lib/keel/caddy-reload.sh.new /var/lib/keel/caddy-reload.sh
/var/lib/keel/caddy-reload.sh || fail "Keel could not send $host to the app."
lock=waiting
code=$(curl -sk --max-time 5 -o /dev/null -w '%{http_code}' --resolve "$host:443:$house" "https://$host/" || true)
case "$code" in
  2*|3*) lock=locked ;;
esac
printf '%s\n' "$lock"
exit 0
