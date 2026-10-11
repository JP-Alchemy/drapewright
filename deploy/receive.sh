#!/usr/bin/env bash
# drapewright-receive: installs a release of the Drapewright web app. It is the
# ONLY command the CI deploy key may run on the server (forced in root's
# authorized_keys by add-ci-key.sh); deploy.sh uses it too. setup-server.sh
# installs it as /usr/local/bin/drapewright-receive with the domain filled in.
#
# It reads a .tgz of dist/ (the built app) from stdin. It checks the release
# holds only plain files under dist/, then unpacks it beside the earlier
# releases and switches the site to it in one step. It confirms the site serves
# it, and switches back if not. The five newest releases are kept, for rolling
# back by hand. Usage: drapewright-receive [release <git sha>] < release.tgz
set -euo pipefail
umask 022
DOMAIN=__DOMAIN__
ROOT=/var/www/drapewright
RELEASES="$ROOT/releases"

# One release at a time.
exec 9>"$ROOT/.receive.lock"
flock -w 600 9

# Which commit this is, when the caller says ("release <sha>").
REQUEST="${SSH_ORIGINAL_COMMAND:-$*}"
REV=unknown
if [[ "$REQUEST" =~ ^release\ ([0-9a-f]{7,40})$ ]]; then REV="${BASH_REMATCH[1]}"; fi

STAGE=$(mktemp -d "$ROOT/incoming.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
head -c 100000000 > "$STAGE/release.tgz"

LIST=$(tar -tzf "$STAGE/release.tgz")
# Only the app's own files, and no absolute paths or ../ tricks.
if echo "$LIST" | grep -Eq '(^/|(^|/)\.\.(/|$))' || echo "$LIST" | grep -Ev '^dist(/|$)' | grep -q .; then
  echo "release rejected: unexpected paths" >&2
  exit 1
fi
# Plain files and folders only (no symlinks or hard links that could point elsewhere).
if tar -tvzf "$STAGE/release.tgz" | cut -c1 | grep -qv '[-d]'; then
  echo "release rejected: links aren't allowed" >&2
  exit 1
fi
mkdir "$STAGE/app"
tar -xzf "$STAGE/release.tgz" -C "$STAGE/app" --no-same-owner --no-same-permissions
if [ ! -f "$STAGE/app/dist/index.html" ]; then
  echo "release rejected: no index.html" >&2
  exit 1
fi
# The site serves this, so anyone can see which commit is live.
printf '%s\n' "$REV" > "$STAGE/app/dist/version.txt"

mkdir -p "$RELEASES"
NEW="$RELEASES/$(date -u +%Y%m%d-%H%M%S)-${REV:0:12}"
mv "$STAGE/app/dist" "$NEW"
PREVIOUS=$(readlink "$ROOT/current" || true)

switch_to() {
  ln -sfn "$1" "$ROOT/current.next"
  mv -T "$ROOT/current.next" "$ROOT/current"
}

# The site, as visitors reach it (HTTPS once it has a certificate), answers
# with this release's version and page.
serves() {
  local scheme=http port=80
  if [ -e /etc/letsencrypt/live/drapewright/fullchain.pem ]; then scheme=https port=443; fi
  local at="$DOMAIN:$port:127.0.0.1"
  for _ in 1 2 3 4 5; do
    if [ "$(curl -fsS --max-time 5 --resolve "$at" "$scheme://$DOMAIN/version.txt" 2>/dev/null)" = "$REV" ] &&
      curl -fsS --max-time 5 --resolve "$at" -o /dev/null "$scheme://$DOMAIN/"; then
      return 0
    fi
    sleep 1
  done
  return 1
}

echo "== switching to $(basename "$NEW")"
switch_to "$NEW"
if serves; then
  echo "== live: $REV"
  # Keep the five newest releases (by name: they start with the time they
  # arrived), and never the live one.
  LIVE=$(readlink -f "$ROOT/current")
  find "$RELEASES" -mindepth 1 -maxdepth 1 -type d | sort -r | tail -n +6 | while read -r old; do
    if [ "$(readlink -f "$old")" != "$LIVE" ]; then rm -rf -- "$old"; fi
  done
  exit 0
fi

echo "== the site didn't serve the new release: switching back" >&2
if [ -n "$PREVIOUS" ] && [ -d "$PREVIOUS" ]; then
  switch_to "$PREVIOUS"
  echo "== back on $(basename "$PREVIOUS")" >&2
fi
rm -rf -- "$NEW"
exit 1
