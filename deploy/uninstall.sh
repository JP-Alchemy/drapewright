#!/usr/bin/env bash
# Takes Drapewright off the server again (run from the repo, on your machine).
#
#   ./deploy/uninstall.sh          removes the nginx site (the files stay)
#   PURGE=1 ./deploy/uninstall.sh  also deletes the releases, the receive script,
#                                  the CI key and the certificate
#
# Other sites are untouched; nginx is tested before it is reloaded.
set -euo pipefail
SERVER="${SERVER:-root@91.98.193.172}"
ssh "$SERVER" "PURGE='${PURGE:-}' bash -s" <<'REMOTE'
set -euo pipefail
rm -f /etc/nginx/sites-enabled/zz-drapewright.conf /etc/nginx/sites-available/zz-drapewright.conf \
  /etc/nginx/snippets/drapewright-app.conf
nginx -t && systemctl reload nginx
echo "== nginx site removed"
if [ "$PURGE" = 1 ]; then
  rm -rf /var/www/drapewright
  rm -f /usr/local/bin/drapewright-receive
  sed -i '/ drapewright-ci$/d' /root/.ssh/authorized_keys 2>/dev/null || true
  certbot delete --cert-name drapewright --non-interactive 2>/dev/null || true
  echo "== releases, receive script, CI key and certificate removed"
fi
REMOTE
