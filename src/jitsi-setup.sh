set -uo pipefail
name='__KEEL_NAME__'
cfg=/var/lib/jitsi
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/jitsi.json.tmp
  mv /run/keel/jitsi.json.tmp /run/keel/jitsi.json
}
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
join_door() {
  podman network connect keel-jitsi keel-door >"$log" 2>&1 || true
}
mark 8 "Opening Video."
install -d -m 0755 \
  "$cfg/web/keys" \
  "$cfg/web/custom-ca" \
  "$cfg/storage/web" \
  "$cfg/storage/transcripts" \
  "$cfg/storage/prosody" \
  "$cfg/tmp/web-load-test" \
  "$cfg/prosody/config" \
  "$cfg/prosody/prosody-plugins-custom" \
  "$cfg/prosody/custom-ca" \
  "$cfg/jicofo/custom-ca" \
  "$cfg/jvb/custom-ca" \
  /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1 || ! command -v openssl >/dev/null 2>&1; then
  mark 18 "Installing the program that runs Video."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Video. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl openssl >"$log" 2>&1 || fail "Keel could not install Video. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so Video did not start."
keep() {
  if [ -f "$cfg/meet.env" ] && grep -q "^$1=" "$cfg/meet.env"; then
    grep "^$1=" "$cfg/meet.env" | head -n 1 | cut -d= -f2-
  else
    openssl rand -hex 16
  fi
}
jicofo_secret=$(keep JICOFO_COMPONENT_SECRET)
jicofo_pass=$(keep JICOFO_AUTH_PASSWORD)
jvb_pass=$(keep JVB_AUTH_PASSWORD)
jigasi_pass=$(keep JIGASI_XMPP_PASSWORD)
jibri_recorder=$(keep JIBRI_RECORDER_PASSWORD)
jibri_pass=$(keep JIBRI_XMPP_PASSWORD)
umask 077
cat > "$cfg/meet.env" <<EOF
CONFIG=$cfg
TZ=UTC
PUBLIC_URL=https://$name
XMPP_DOMAIN=meet.jitsi
XMPP_AUTH_DOMAIN=auth.meet.jitsi
XMPP_GUEST_DOMAIN=guest.meet.jitsi
XMPP_MUC_DOMAIN=muc.meet.jitsi
XMPP_INTERNAL_MUC_DOMAIN=internal-muc.meet.jitsi
XMPP_HIDDEN_DOMAIN=hidden.meet.jitsi
XMPP_SERVER=xmpp.meet.jitsi
XMPP_BOSH_URL_BASE=http://xmpp.meet.jitsi:5280
JVB_ADVERTISE_IPS=$house
JVB_PORT=10000
JVB_DISABLE_STUN=1
ENABLE_P2P=1
ENABLE_AUTH=0
ENABLE_GUESTS=1
ENABLE_RECORDING=0
ENABLE_TRANSCRIPTIONS=0
ENABLE_LIVESTREAMING=0
ENABLE_LETSENCRYPT=0
DISABLE_HTTPS=1
ENABLE_HTTP_REDIRECT=0
ENABLE_COLIBRI_WEBSOCKET=1
ENABLE_XMPP_WEBSOCKET=1
PROSODY_RESERVATION_ENABLED=0
JICOFO_MAX_MEMORY=256m
VIDEOBRIDGE_MAX_MEMORY=512m
JICOFO_COMPONENT_SECRET=$jicofo_secret
JICOFO_AUTH_PASSWORD=$jicofo_pass
JVB_AUTH_PASSWORD=$jvb_pass
JIGASI_XMPP_PASSWORD=$jigasi_pass
JIBRI_RECORDER_PASSWORD=$jibri_recorder
JIBRI_XMPP_PASSWORD=$jibri_pass
EOF
chmod 0600 "$cfg/meet.env" || fail "Keel could not store the meeting settings."
want="$name $house"
page_up() {
  curl -fsS -o /dev/null --max-time 3 "http://127.0.0.1:8080/"
}
all_up() {
  cstate=$(podman inspect -f '{{.State.Running}}' keel-jitsi-prosody 2>/dev/null || echo false)
  [ "$cstate" = "true" ] || return 1
  cstate=$(podman inspect -f '{{.State.Running}}' keel-jitsi-jicofo 2>/dev/null || echo false)
  [ "$cstate" = "true" ] || return 1
  cstate=$(podman inspect -f '{{.State.Running}}' keel-jitsi-jvb 2>/dev/null || echo false)
  [ "$cstate" = "true" ] || return 1
  cstate=$(podman inspect -f '{{.State.Running}}' keel-jitsi-web 2>/dev/null || echo false)
  [ "$cstate" = "true" ] || return 1
  page_up
}
if [ "$(cat "$cfg/stamp" 2>/dev/null || true)" = "$want" ] && all_up; then
  join_door
  touch "$cfg/installed"
  exit 0
fi
missing=0
for image in \
  ghcr.io/jitsi/web:__KEEL_TAG__ \
  ghcr.io/jitsi/prosody:__KEEL_TAG__ \
  ghcr.io/jitsi/jicofo:__KEEL_TAG__ \
  ghcr.io/jitsi/jvb:__KEEL_TAG__
do
  podman image exists "$image" >/dev/null 2>&1 || missing=1
done
if [ "$missing" = "1" ]; then
  mark 55 "Downloading Video. This can take a while."
  for image in \
    ghcr.io/jitsi/web:__KEEL_TAG__ \
    ghcr.io/jitsi/prosody:__KEEL_TAG__ \
    ghcr.io/jitsi/jicofo:__KEEL_TAG__ \
    ghcr.io/jitsi/jvb:__KEEL_TAG__
  do
    podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Video. Check the network and try again."
  done
else
  mark 55 "Opening Video."
fi
mark 78 "Starting the meeting."
podman network exists keel-jitsi >/dev/null 2>&1 || podman network create keel-jitsi >"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
podman rm -f keel-jitsi-web keel-jitsi-jvb keel-jitsi-jicofo keel-jitsi-prosody >"$log" 2>&1 || true
podman run -d --name keel-jitsi-prosody --hostname prosody \
  --network keel-jitsi --network-alias xmpp.meet.jitsi --network-alias prosody \
  --env-file "$cfg/meet.env" --read-only \
  --tmpfs /run:rw,exec,nosuid,size=32m --tmpfs /tmp:rw,noexec,nosuid,size=16m \
  -v "$cfg/prosody/config:/config" \
  -v "$cfg/prosody/prosody-plugins-custom:/prosody-plugins-custom" \
  -v "$cfg/storage/prosody:/var/lib/prosody" \
  -v "$cfg/prosody/custom-ca:/usr/local/share/ca-certificates:ro" \
  ghcr.io/jitsi/prosody:__KEEL_TAG__ >"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
podman run -d --name keel-jitsi-jicofo --hostname jicofo \
  --network keel-jitsi --network-alias jicofo \
  --env-file "$cfg/meet.env" --read-only \
  --tmpfs /run:rw,exec,nosuid,size=32m --tmpfs /tmp:rw,noexec,nosuid,size=16m \
  -v "$cfg/jicofo:/config" \
  -v "$cfg/jicofo/custom-ca:/usr/local/share/ca-certificates:ro" \
  ghcr.io/jitsi/jicofo:__KEEL_TAG__ >"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
podman run -d --name keel-jitsi-jvb --hostname jvb \
  --network keel-jitsi --network-alias jvb \
  --env-file "$cfg/meet.env" --read-only \
  -p 10000:10000/udp \
  --tmpfs /run:rw,exec,nosuid,size=32m --tmpfs /tmp:rw,noexec,nosuid,size=16m \
  -v "$cfg/jvb:/config" \
  -v "$cfg/jvb/custom-ca:/usr/local/share/ca-certificates:ro" \
  ghcr.io/jitsi/jvb:__KEEL_TAG__ >"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
podman run -d --name keel-jitsi-web --hostname web \
  --network keel-jitsi --network-alias web \
  --env-file "$cfg/meet.env" --read-only \
  -p 127.0.0.1:8080:80 \
  --tmpfs /run:rw,exec,nosuid,size=32m --tmpfs /tmp:rw,noexec,nosuid,size=16m \
  -v "$cfg/web:/config" \
  -v "$cfg/storage/web:/storage" \
  -v "$cfg/web/custom-ca:/usr/local/share/ca-certificates:ro" \
  -v "$cfg/storage/transcripts:/usr/share/jitsi-meet/transcripts:ro" \
  -v "$cfg/tmp/web-load-test:/usr/share/jitsi-meet/load-test" \
  ghcr.io/jitsi/web:__KEEL_TAG__ >"$log" 2>&1 || fail "Keel could not start Video. Try Open again."
mark 92 "Waiting for the meeting page."
up=0
i=0
while [ "$i" -lt 40 ]; do
  if page_up; then
    up=1
    break
  fi
  i=$((i + 1))
  sleep 2
done
[ "$up" = "1" ] || fail "Video started, but the meeting page did not answer. Try Open again."
join_door
printf '%s\n' "$want" > "$cfg/stamp"
touch "$cfg/installed"
exit 0
