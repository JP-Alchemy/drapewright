#!/usr/bin/env bash
# Builds Drapewright and ships it to your server (run from the repo, on your machine).
#
#   CHECK=1 ./deploy/deploy.sh    look at the server first and report (changes nothing)
#   ./deploy/deploy.sh            deploy (defaults: SERVER=root@91.98.193.172, DOMAIN=drapewright.com)
#
# Until the site has its HTTPS certificate, every run also sets the server up
# (see setup-server.sh): an nginx site for $DOMAIN only, the release folders,
# and the certificate once DNS points at the server. Other sites on the server
# are left alone. Optional: EMAIL (certificate notices), SETUP=1 to redo setup.
# To remove it again: ./deploy/uninstall.sh
set -euo pipefail
SERVER="${SERVER:-root@91.98.193.172}"
DOMAIN="${DOMAIN:-drapewright.com}"
cd "$(dirname "$0")/.."

upload_setup() {
  ssh "$SERVER" 'mkdir -p /tmp/drapewright'
  scp -q deploy/setup-server.sh deploy/receive.sh deploy/nginx-drapewright.conf \
    deploy/nginx-drapewright-http.conf deploy/nginx-drapewright-app.conf "$SERVER":/tmp/drapewright/
}

if [ "${CHECK:-}" = 1 ]; then
  upload_setup
  ssh "$SERVER" "CHECK=1 DOMAIN='$DOMAIN' bash /tmp/drapewright/setup-server.sh"
  exit 0
fi

echo "== build"
pnpm install --frozen-lockfile
pnpm wasm
pnpm --filter @drapewright/web build

if [ "${SETUP:-}" = 1 ] || ! ssh "$SERVER" 'test -x /usr/local/bin/drapewright-receive && test -e /etc/letsencrypt/live/drapewright/fullchain.pem'; then
  echo "== server setup"
  upload_setup
  ssh "$SERVER" "DOMAIN='$DOMAIN' EMAIL='${EMAIL:-}' bash /tmp/drapewright/setup-server.sh"
fi

REV=$(git rev-parse HEAD)
[ -z "$(git status --porcelain)" ] || echo "(note: uncommitted changes are included; the release is labelled $REV)"
echo "== ship $REV"
# The release is dist/ only; _headers is for other hosts. macOS's tar would add
# its file metadata, which the server's tar doesn't know.
TAR_OPTS=()
if tar --version | grep -q bsdtar; then TAR_OPTS=(--no-mac-metadata --no-xattrs); fi
COPYFILE_DISABLE=1 tar "${TAR_OPTS[@]}" -C web -czf - --exclude=dist/_headers dist |
  ssh "$SERVER" "/usr/local/bin/drapewright-receive release $REV"

if ssh "$SERVER" 'test -e /etc/letsencrypt/live/drapewright/fullchain.pem'; then
  curl -fsS "https://$DOMAIN/version.txt" >/dev/null && echo "Live: https://$DOMAIN"
else
  echo "Live over plain HTTP until DNS points here: http://$DOMAIN"
fi
