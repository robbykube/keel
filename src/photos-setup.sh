set -uo pipefail
owned='__KEEL_OWNED__'
cfg=/var/lib/immich
photos=/srv/keel/files/Media/Photos
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/photos.json.tmp
  mv /run/keel/photos.json.tmp /run/keel/photos.json
}
json_access() {
  printf '%s' "$1" | sed -n 's/.*"accessToken"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1
}
mark 8 "Opening Photos."
if [ ! -d /srv/keel/files/Media ]; then
  fail "Set up the Files disk first. Pictures live in Media."
fi
install -d -m 0755 "$photos" "$cfg/library" "$cfg/cache" "$cfg/postgres" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1 || ! command -v openssl >/dev/null 2>&1; then
  mark 16 "Installing the program that shows pictures."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Photos. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl openssl >"$log" 2>&1 || fail "Keel could not install Photos. Try Open again."
fi
if [ -f "$cfg/db.pass" ]; then
  dbpass=$(cat "$cfg/db.pass")
else
  dbpass=$(openssl rand -hex 16)
  umask 077
  printf '%s\n' "$dbpass" > "$cfg/db.pass"
  chmod 0600 "$cfg/db.pass"
fi
server=ghcr.io/immich-app/immich-server:v3.2.2
learn=ghcr.io/immich-app/immich-machine-learning:v3.2.2
cache=docker.io/valkey/valkey:9@sha256:70739f85ad2ee01a726a965584a0f94895f01b0c60b3cc8b0aeef11eaa6888cf
db=ghcr.io/immich-app/postgres:14-vectorchord0.4.3-pgvectors0.2.0@sha256:bcf63357191b76a916ae5eb93464d65c07511da41e3bf7a8416db519b40b1c23
missing=0
for image in "$server" "$learn" "$cache" "$db"; do
  podman image exists "$image" >/dev/null 2>&1 || missing=1
done
if [ "$missing" = 1 ]; then
  mark 40 "Downloading Photos. This can take a while."
  for image in "$server" "$learn" "$cache" "$db"; do
    podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Photos. Check the network and try again."
  done
else
  mark 40 "Opening Photos."
fi
mark 70 "Starting Photos."
podman network exists keel-photos >/dev/null 2>&1 || podman network create keel-photos >"$log" 2>&1 || fail "Keel could not start Photos. Try Open again."
podman rm -f keel-photos-server keel-photos-ml keel-photos-db keel-photos-redis >"$log" 2>&1 || true
podman run -d --name keel-photos-redis --network keel-photos --network-alias redis \
  "$cache" >"$log" 2>&1 || fail "Keel could not start Photos. Try Open again."
podman run -d --name keel-photos-db --network keel-photos --network-alias database \
  --shm-size=128m \
  -e POSTGRES_PASSWORD="$dbpass" \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=immich \
  -e POSTGRES_INITDB_ARGS='--data-checksums' \
  -v "$cfg/postgres:/var/lib/postgresql/data" \
  "$db" >"$log" 2>&1 || fail "Keel could not start Photos. Try Open again."
podman run -d --name keel-photos-ml --network keel-photos --network-alias immich-machine-learning \
  -e TZ=UTC \
  -v "$cfg/cache:/cache" \
  "$learn" >"$log" 2>&1 || fail "Keel could not start Photos. Try Open again."
podman run -d --name keel-photos-server --network keel-photos --network-alias immich-server \
  -p 127.0.0.1:2283:2283 \
  -e DB_PASSWORD="$dbpass" \
  -e DB_USERNAME=postgres \
  -e DB_DATABASE_NAME=immich \
  -e DB_HOSTNAME=database \
  -e REDIS_HOSTNAME=redis \
  -e IMMICH_MACHINE_LEARNING_URL=http://immich-machine-learning:3003 \
  -e TZ=UTC \
  -v "$cfg/library:/data" \
  -v "$photos:/photos:ro" \
  "$server" >"$log" 2>&1 || fail "Keel could not start Photos. Try Open again."
mark 82 "Waiting for the pictures page."
up=0
i=0
while [ "$i" -lt 60 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:2283/api/server/ping" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = 1 ] || fail "Photos started, but the page did not answer. Try Open again."
podman network connect keel-photos keel-door >"$log" 2>&1 || true
if [ ! -f "$cfg/account.pass" ]; then
  account=$(openssl rand -hex 12)
  umask 077
  printf '%s\n' "$account" > "$cfg/account.pass"
  chmod 0600 "$cfg/account.pass"
  curl -sS -o /dev/null --max-time 15 -X POST "http://127.0.0.1:2283/api/auth/admin-sign-up" \
    -H 'Content-Type: application/json' \
    -d "{\"email\":\"photos@$owned\",\"password\":\"$account\",\"name\":\"Photos\"}" || true
else
  account=$(cat "$cfg/account.pass")
fi
session=$(curl -sS --max-time 15 -X POST "http://127.0.0.1:2283/api/auth/login" -H 'Content-Type: application/json' -d "{\"email\":\"photos@$owned\",\"password\":\"$account\"}" || true)
access=$(json_access "$session")
if [ -n "$access" ]; then
  libcode=$(curl -sS -o /dev/null --max-time 15 -w '%{http_code}' -X POST "http://127.0.0.1:2283/api/libraries" \
    -H "Authorization: Bearer $access" -H 'Content-Type: application/json' \
    -d '{"name":"Media","importPaths":["/photos"],"exclusionPatterns":[]}' || true)
  case "$libcode" in
    2*) touch "$cfg/library.ready" ;;
  esac
fi
cat > /etc/systemd/system/keel-photos.service <<'EOF'
[Unit]
Description=Keel photos
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-photos-redis
ExecStart=/usr/bin/podman start keel-photos-db
ExecStart=/usr/bin/podman start keel-photos-ml
ExecStart=/usr/bin/podman start keel-photos-server
ExecStop=/usr/bin/podman stop -t 20 keel-photos-server
ExecStop=/usr/bin/podman stop -t 20 keel-photos-ml
ExecStop=/usr/bin/podman stop -t 20 keel-photos-db
ExecStop=/usr/bin/podman stop -t 10 keel-photos-redis
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-photos.service >"$log" 2>&1 || true
touch "$cfg/installed"
[ -n "$account" ] || fail "Keel could not make the Photos sign-in."
included=no
[ -f "$cfg/library.ready" ] && included=yes
printf 'photos@%s|%s|%s\n' "$owned" "$account" "$included"
exit 0
