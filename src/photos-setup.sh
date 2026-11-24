set -uo pipefail
owned='__KEEL_OWNED__'
cfg=/var/lib/immich
photos=/srv/keel/files/Media/Photos
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
# Temporary space filled up, and a new program could not start.
make_room() {
  avail=$(df -Pk /run 2>/dev/null | awk 'NR==2 { print $4 }')
  case "$avail" in
    ''|*[!0-9]*) return 0 ;;
  esac
  if [ "$avail" -ge 32768 ]; then
    return 0
  fi
  journalctl --vacuum-size=8M >/dev/null 2>&1 || true
  rm -rf /run/log/journal /run/log/journal.* >/dev/null 2>&1 || true
  avail=$(df -Pk /run 2>/dev/null | awk 'NR==2 { print $4 }')
  case "$avail" in
    ''|*[!0-9]*) return 0 ;;
  esac
  if [ "$avail" -lt 16384 ]; then
    fail "The Keel computer ran out of room to start Photos. Restart it, then press Open again."
  fi
}
start_fail() {
  last=$(sed '/^$/d' "$log" 2>/dev/null | tail -n 1 | cut -c 1-140)
  case "$last" in
    *[Nn]o\ space*|*[Nn]ot\ enough\ space*)
      fail "The Keel computer ran out of room to start Photos. Restart it, then press Open again."
      ;;
  esac
  fail "${last:-Keel could not start Photos. Try Open again.}"
}
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
# The pictures are already in Media. This download is the program that shows them.
pull_note() {
  mark "$1" "Downloading the Photos program, part $2 of 4. Your pictures stay in Media."
}
pull_program() {
  n=0
  for image in "$server" "$learn" "$cache" "$db"; do
    n=$((n + 1))
    case "$n" in
      1) base=40; span=10 ;;
      2) base=50; span=14 ;;
      3) base=64; span=2 ;;
      *) base=66; span=2 ;;
    esac
    if podman image exists "$image" >/dev/null 2>&1; then
      pull_note $((base + span)) "$n"
      continue
    fi
    pull_note "$base" "$n"
    layers=0
    seen=$base
    last=0
    if ! script -q -e -c "podman pull $(printf '%q' "$image")" "$cfg/pull.log" 2>&1 | tr '\r' '\n' | while IFS= read -r line; do
      printf '%s\n' "$line" >>"$log"
      parsed=$(printf '%s\n' "$line" | awk -v layers="$layers" -v span="$span" '
        function bytes(n, u) {
          if (u ~ /^Gi/) return n * 1073741824
          if (u ~ /^Mi/) return n * 1048576
          if (u ~ /^Ki/) return n * 1024
          if (u ~ /^G/) return n * 1000000000
          if (u ~ /^M/) return n * 1000000
          if (u ~ /^K/) return n * 1000
          return n + 0
        }
        BEGIN { frac = 0; done = 0 }
        {
          if ($0 ~ /done[[:space:]]*$/) done = 1
          if (match($0, /[0-9.]+[[:space:]]*[KMGT]i?B[[:space:]]*\/[[:space:]]*[0-9.]+[[:space:]]*[KMGT]i?B/)) {
            s = substr($0, RSTART, RLENGTH)
            split(s, p, /[[:space:]]*\/[[:space:]]*/)
            split(p[1], g, /[[:space:]]+/)
            split(p[2], t, /[[:space:]]+/)
            tb = bytes(t[1], t[2])
            if (tb > 0) frac = bytes(g[1], g[2]) / tb
            if (frac > 1) frac = 1
          }
        }
        END {
          fake = layers + frac
          if (done) fake = layers + 1
          pct = int(span * fake / (fake + 6))
          if (pct >= span) pct = span - 1
          if (pct < 0) pct = 0
          printf "%d %d\n", pct, done
        }
      ')
      pct=${parsed%% *}
      done=${parsed##* }
      case "$pct" in
        ''|*[!0-9]*) pct=0 ;;
      esac
      if [ "$done" = 1 ]; then
        layers=$((layers + 1))
      fi
      step=$((base + pct))
      now=$(date +%s)
      if [ "$step" -gt "$seen" ] && [ "$now" != "$last" ]; then
        seen=$step
        last=$now
        pull_note "$seen" "$n"
      fi
    done; then
      fail "Keel could not download the Photos program. Check the network and try again."
    fi
    pull_note $((base + span)) "$n"
  done
}
if [ "$missing" = 1 ]; then
  if ! command -v script >/dev/null 2>&1; then
    mark 40 "Downloading the Photos program. Your pictures stay in Media."
    for image in "$server" "$learn" "$cache" "$db"; do
      podman pull "$image" >"$log" 2>&1 || fail "Keel could not download the Photos program. Check the network and try again."
    done
  else
    pull_program
  fi
else
  mark 40 "Opening Photos."
fi
make_room
mark 70 "Starting Photos."
podman network exists keel-photos >/dev/null 2>&1 || podman network create keel-photos >"$log" 2>&1 || start_fail
podman rm -f keel-photos-server keel-photos-ml keel-photos-db keel-photos-redis >"$log" 2>&1 || true
podman run -d --name keel-photos-redis --network keel-photos --network-alias redis \
  "$cache" >"$log" 2>&1 || start_fail
podman run -d --name keel-photos-db --network keel-photos --network-alias database \
  --shm-size=128m \
  -e POSTGRES_PASSWORD="$dbpass" \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=immich \
  -e POSTGRES_INITDB_ARGS='--data-checksums' \
  -v "$cfg/postgres:/var/lib/postgresql/data" \
  "$db" >"$log" 2>&1 || start_fail
podman run -d --name keel-photos-ml --network keel-photos --network-alias immich-machine-learning \
  -e TZ=UTC \
  -v "$cfg/cache:/cache" \
  "$learn" >"$log" 2>&1 || start_fail
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
  "$server" >"$log" 2>&1 || start_fail
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
systemctl daemon-reload >>"$log" 2>&1 || fail "Keel could not load the Photos service."
systemctl enable keel-photos.service >>"$log" 2>&1 || fail "Keel could not enable Photos to stay on."
# Move the containers out of this temporary setup service's cgroup before
# handing ownership to the persistent system service.
podman stop -t 20 keel-photos-server keel-photos-ml keel-photos-db keel-photos-redis >>"$log" 2>&1 || true
systemctl restart keel-photos.service >>"$log" 2>&1 || fail "Keel could not keep Photos running. Check Logs."
touch "$cfg/installed"
[ -n "$account" ] || fail "Keel could not make the Photos sign-in."
included=no
[ -f "$cfg/library.ready" ] && included=yes
printf 'photos@%s|%s|%s\n' "$owned" "$account" "$included"
exit 0
