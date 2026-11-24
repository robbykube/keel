set -uo pipefail
# Light Mail: SnappyMail, a web mail page of about 100 MB, as keel-mail on
# 127.0.0.1:3000, the same address and door as the full Mail. People sign
# in with their own email address and its password. Keel writes the
# settings for the common providers, so that is all it takes.
host='__KEEL_HOST__'
cfg=/var/lib/snappymail
log=/var/lib/keel-mail-light.log
image=docker.io/djmaze/snappymail:v2.38.2
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/mail.json.tmp
  mv /run/keel/mail.json.tmp /run/keel/mail.json
}
mark 8 "Opening Mail."
: > "$log"
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 16 "Installing the program that runs Mail."
  apt-get update -o Acquire::ForceIPv4=true >>"$log" 2>&1 || fail "Keel could not download Mail. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >>"$log" 2>&1 || fail "Keel could not install Mail. Try Open again."
fi
install -d -m 0750 "$cfg"
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 40 "Downloading Mail."
  podman pull "$image" >>"$log" 2>&1 || fail "Keel could not download Mail. Check the network and try again."
fi
mark 70 "Starting Mail."
# The full Mail (Thunderbird) has the same name and address.
podman rm -f keel-mail >>"$log" 2>&1 || true
podman network exists keel-mail >/dev/null 2>&1 || podman network create keel-mail >>"$log" 2>&1 || fail "Keel could not start Mail. Try Open again."
if ! podman run -d --name keel-mail --hostname mail \
  --network keel-mail --network-alias mail \
  --memory 256m \
  -e MEMORY_LIMIT=96M \
  -e UPLOAD_MAX_SIZE=25M \
  -e SECURE_COOKIES=true \
  -p 127.0.0.1:3000:8888 \
  -v "$cfg:/var/lib/snappymail" \
  "$image" >>"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on that address. Mail did not start."
  fi
  fail "Keel could not start Mail. Try Open again."
fi
mark 85 "Waiting for the mail page."
up=0
i=0
while [ "$i" -lt 60 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:3000/" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = 1 ] || fail "Mail started, but the page did not answer. Try Open again."

# The common providers. type 1 is SSL from the start, 2 is STARTTLS.
domains="$cfg/_data_/_default_/domains"
install -d "$domains"
provider() {
  imap_host=$1 imap_port=$2 imap_type=$3 smtp_host=$4 smtp_port=$5 smtp_type=$6
  shift 6
  for domain in "$@"; do
    cat > "$domains/$domain.json" <<EOF
{
  "IMAP": { "host": "$imap_host", "port": $imap_port, "type": $imap_type, "timeout": 300, "shortLogin": false, "lowerLogin": true,
    "sasl": ["SCRAM-SHA-256", "SCRAM-SHA-1", "PLAIN", "LOGIN"],
    "ssl": { "verify_peer": true, "verify_peer_name": true, "allow_self_signed": false, "SNI_enabled": true, "disable_compression": true, "security_level": 1 },
    "use_expunge_all_on_delete": false, "fast_simple_search": true, "force_select": false, "message_all_headers": false, "message_list_limit": 10000, "search_filter": "" },
  "SMTP": { "host": "$smtp_host", "port": $smtp_port, "type": $smtp_type, "timeout": 60, "shortLogin": false, "lowerLogin": true,
    "sasl": ["SCRAM-SHA-256", "SCRAM-SHA-1", "PLAIN", "LOGIN"],
    "ssl": { "verify_peer": true, "verify_peer_name": true, "allow_self_signed": false, "SNI_enabled": true, "disable_compression": true, "security_level": 1 },
    "useAuth": true, "setSender": false, "usePhpMail": false },
  "Sieve": { "host": "", "port": 4190, "type": 0, "timeout": 10, "shortLogin": false, "lowerLogin": true, "sasl": ["PLAIN", "LOGIN"],
    "ssl": { "verify_peer": true, "verify_peer_name": true, "allow_self_signed": false, "SNI_enabled": true, "disable_compression": true, "security_level": 1 },
    "enabled": false },
  "whiteList": ""
}
EOF
  done
}
provider imap.gmail.com 993 1 smtp.gmail.com 465 1 gmail.com googlemail.com
# Not Outlook or Hotmail: Microsoft accepts only its own sign-in (OAuth)
# for mail programs since 2024, which the full Mail (Thunderbird) has.
provider imap.mail.yahoo.com 993 1 smtp.mail.yahoo.com 465 1 yahoo.com ymail.com
provider imap.mail.me.com 993 1 smtp.mail.me.com 587 2 icloud.com me.com mac.com
provider imap.aol.com 993 1 smtp.aol.com 465 1 aol.com
podman exec keel-mail chown -R www-data:www-data /var/lib/snappymail/_data_/_default_/domains >>"$log" 2>&1 || true

# The admin page's password: SnappyMail writes it when its admin page is
# first opened.
pass_file="$cfg/_data_/_default_/admin_password.txt"
if [ ! -s "$pass_file" ]; then
  curl -sS -o /dev/null --max-time 10 "http://127.0.0.1:3000/?admin" >>"$log" 2>&1 || true
  i=0
  while [ ! -s "$pass_file" ] && [ "$i" -lt 10 ]; do sleep 1; i=$((i + 1)); done
fi
adminpass=$(tr -d '[:space:]' < "$pass_file" 2>/dev/null || true)
[ -n "$adminpass" ] || adminpass=none

podman network connect keel-mail keel-door >>"$log" 2>&1 || true
cat > /etc/systemd/system/keel-mail.service <<'EOF'
[Unit]
Description=Keel mail
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-mail
ExecStop=/usr/bin/podman stop -t 20 keel-mail
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >>"$log" 2>&1 || fail "Keel could not load the Mail service."
systemctl enable keel-mail.service >>"$log" 2>&1 || fail "Keel could not enable Mail to stay on."
podman stop -t 20 keel-mail >>"$log" 2>&1 || true
systemctl restart keel-mail.service >>"$log" 2>&1 || fail "Keel could not keep Mail running. Check Logs."
touch "$cfg/installed"
printf 'ready|%s\n' "$adminpass"
exit 0
