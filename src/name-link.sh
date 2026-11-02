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
install -d -m 0755 /run/keel
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so the name was not written."
login=$(curl -sS -X POST "http://$house:5380/api/user/login" --data "user=admin&pass=admin" || true)
token=$(json_token "$login")
[ -n "$token" ] || fail "Keel could not sign in to Names, so the name was not written."
etoken=$(urlenc "$token")
curl -sS "http://$house:5380/api/zones/create?token=$etoken&zone=$owned&type=Primary" >/dev/null 2>&1 || true
record=$(curl -sS "http://$house:5380/api/zones/records/add?token=$etoken&domain=$host&zone=$owned&type=A&ipAddress=$house&ttl=60&overwrite=true" || true)
printf '%s' "$record" | grep -Eq '"status"[[:space:]]*:[[:space:]]*"ok"' || fail "Keel could not write $host."
door=$(curl -sS -X POST "http://$house:81/api/tokens" -H 'Content-Type: application/json' -d '{"identity":"admin@example.com","secret":"changeme"}' || true)
dtoken=$(json_token "$door")
[ -n "$dtoken" ] || fail "Keel could not sign in to the front door, so the name was not sent."
proxy='{"domain_names":["'"$host"'"],"forward_scheme":"http","forward_host":"'"$forward"'","forward_port":'"$port"',"access_list_id":0,"certificate_id":"new","ssl_forced":true,"meta":{"letsencrypt_email":"admin@'"$owned"'","letsencrypt_agree":true,"dns_challenge":false},"advanced_config":"","locations":[],"block_exploits":false,"caching_enabled":false,"allow_websocket_upgrade":true,"http2_support":true,"hsts_enabled":false,"hsts_subdomains":false}'
plain='{"domain_names":["'"$host"'"],"forward_scheme":"http","forward_host":"'"$forward"'","forward_port":'"$port"',"access_list_id":0,"certificate_id":0,"ssl_forced":false,"meta":{"letsencrypt_agree":false,"dns_challenge":false},"advanced_config":"","locations":[],"block_exploits":false,"caching_enabled":false,"allow_websocket_upgrade":true,"http2_support":true,"hsts_enabled":false,"hsts_subdomains":false}'
code=$(curl -sS -o /run/keel/door-reply -w '%{http_code}' -X POST "http://$house:81/api/nginx/proxy-hosts" -H "Authorization: Bearer $dtoken" -H 'Content-Type: application/json' -d "$proxy" || true)
lock=waiting
case "$code" in
  2*) lock=locked ;;
  *)
    code=$(curl -sS -o /run/keel/door-reply -w '%{http_code}' -X POST "http://$house:81/api/nginx/proxy-hosts" -H "Authorization: Bearer $dtoken" -H 'Content-Type: application/json' -d "$plain" || true)
    case "$code" in
      2*) lock=waiting ;;
      *)
        list=$(curl -sS "http://$house:81/api/nginx/proxy-hosts" -H "Authorization: Bearer $dtoken" || true)
        printf '%s' "$list" | grep -Fq "$host" || fail "Keel could not send $host to the app."
        lock=locked
        ;;
    esac
    ;;
esac
rm -f /run/keel/door-reply
printf '%s\n' "$lock"
exit 0
