set -uo pipefail
cfg=/var/lib/jellyfin
media=/srv/keel/files/Media
log="$cfg/setup.log"
image=docker.io/jellyfin/jellyfin:10.10.7
fail() { echo "$1" >&2; exit 1; }
if [ ! -d "$media" ]; then
  fail "Set up the Files disk first. Films, shows, and music live in Media."
fi
install -d -m 0755 "$cfg" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Watch. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install Watch. Try Open again."
fi
podman image exists "$image" >/dev/null 2>&1 || podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Watch. Check the network and try again."
podman network exists keel-jellyfin >/dev/null 2>&1 || podman network create keel-jellyfin >"$log" 2>&1 || fail "Keel could not start Watch. Try Open again."
podman rm -f keel-jellyfin >/dev/null 2>&1 || true
podman run -d --name keel-jellyfin --memory=2g --restart unless-stopped \
  --network keel-jellyfin \
  -p 127.0.0.1:8096:8096 \
  -v "$cfg:/config" \
  -v "$media:/media:ro" \
  "$image" >"$log" 2>&1 || fail "Keel could not start Watch. Try Open again."
podman network connect keel-jellyfin keel-door >"$log" 2>&1 || true
ok=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 http://127.0.0.1:8096/web/ || true)
  case "$code" in
    2*|3*) ok=1; break ;;
  esac
  sleep 2
done
[ "$ok" = 1 ] || fail "Watch did not answer. Try Open again."
touch "$cfg/installed"
