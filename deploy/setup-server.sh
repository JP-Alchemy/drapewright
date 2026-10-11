#!/usr/bin/env bash
# Sets up Drapewright on a Debian/Ubuntu server with nginx that may already host
# other sites (run as root, through deploy.sh). It is built to leave them alone:
#
# - nginx: one site file that only answers for $DOMAIN and www.$DOMAIN, plus a
#   snippet, with their own variable names. Other sites' files are never
#   edited.
#   - The file is named to load after the others, so it never becomes nginx's
#     default site; the default is checked before and after.
#   - The config is tested before anything is added. If nginx rejects the new
#     file, or the default site would change, it is removed again and nginx is
#     not reloaded. Reloads are graceful.
# - certbot: a certificate named "drapewright" for $DOMAIN and www.$DOMAIN only,
#   through a webroot, so certbot never edits nginx files. Renewals (the
#   existing certbot timer) reload nginx. Until DNS points here, the site runs
#   on plain HTTP, and every run tries again.
# - Files: the app's releases in /var/www/drapewright, and
#   /usr/local/bin/drapewright-receive (receive.sh), which installs them.
#
# CHECK=1 only looks around and reports (changes nothing). Safe to run again.
set -euo pipefail
DOMAIN="${DOMAIN:-drapewright.com}"
EMAIL="${EMAIL:-}"
SRC=/tmp/drapewright
ROOT=/var/www/drapewright
CERT_NAME=drapewright
CERT=/etc/letsencrypt/live/$CERT_NAME/fullchain.pem
# Loads after the other sites' files (they load in name order).
SITE_FILE=/etc/nginx/sites-available/zz-drapewright.conf
SITE_LINK=/etc/nginx/sites-enabled/zz-drapewright.conf
SNIPPET=/etc/nginx/snippets/drapewright-app.conf
RECEIVE=/usr/local/bin/drapewright-receive

this_ip() { ip -4 -o addr show scope global | awk '{print $4}' | cut -d/ -f1 | head -1; }
this_ip6() { ip -6 -o addr show scope global | awk '{print $4}' | cut -d/ -f1 | head -1; }
dns_of() { getent ahostsv4 "$1" | awk '{print $1}' | sort -u | xargs; }
dns6_of() { getent ahostsv6 "$1" | awk '{print $1}' | grep -v '^::ffff:' | sort -u | xargs; }
# A record here, and AAAA (optional) here too: Let's Encrypt checks over IPv6
# when there is one.
points_here() {
  [ "$(dns_of "$1")" = "$(this_ip)" ] && { [ -z "$(dns6_of "$1")" ] || [ "$(dns6_of "$1")" = "$(this_ip6)" ]; }
}
# The server nginx answers with for names it doesn't know: the first one loaded.
default_site() { nginx -T 2>/dev/null | grep -m1 -E '^\s*server_name' | xargs; }

if [ "${CHECK:-}" = 1 ]; then
  set +e +o pipefail
  echo "== Read-only check (nothing is changed)"
  echo "-- server: $(. /etc/os-release && echo "$PRETTY_NAME"), disk free $(df -h / | awk 'NR==2 {print $4}')"
  echo "-- nginx: $(nginx -v 2>&1)"
  nginx -t 2>&1 | sed 's/^/   /'
  echo "-- sites nginx serves now (server_name):"
  nginx -T 2>/dev/null | grep -E '^\s*server_name' | sort -u | sed 's/^\s*/   /'
  echo "-- default site (stays as it is): $(default_site)"
  echo "-- DNS: $DOMAIN -> $(dns_of "$DOMAIN") $(dns6_of "$DOMAIN"), www.$DOMAIN -> $(dns_of "www.$DOMAIN") $(dns6_of "www.$DOMAIN") (this server: $(this_ip) $(this_ip6))"
  echo "-- certbot: $(certbot --version 2>&1); certificate $CERT_NAME: $([ -e "$CERT" ] && echo yes || echo 'not yet')"
  echo "-- site file: $([ -e "$SITE_LINK" ] && echo "$SITE_LINK" || echo "not yet (would add $SITE_LINK)")"
  live=$(readlink "$ROOT/current" 2>/dev/null)
  if [ -n "$live" ]; then
    echo "-- live release: $(basename "$live") (version $(cat "$ROOT/current/version.txt" 2>/dev/null))"
  else
    echo "-- live release: none yet"
  fi
  exit 0
fi

# Writes the site file from a template, keeping nginx working and the default
# site unchanged; puts the old file back if anything is wrong.
install_site() {
  local template=$1 before backup=""
  before=$(default_site)
  if [ -e "$SITE_FILE" ]; then backup=$(mktemp) && cp "$SITE_FILE" "$backup"; fi
  sed "s/__DOMAIN__/$DOMAIN/g" "$template" > "$SITE_FILE"
  ln -sf "$SITE_FILE" "$SITE_LINK"
  if nginx -t 2>/dev/null && [ "$(default_site)" = "$before" ]; then
    systemctl reload nginx
    rm -f "$backup"
    return 0
  fi
  echo "nginx rejected the site, or the default site would change: putting things back (nothing was reloaded)" >&2
  nginx -t || true
  if [ -n "$backup" ]; then
    cp "$backup" "$SITE_FILE"
    rm -f "$backup"
  else
    rm -f "$SITE_LINK" "$SITE_FILE"
  fi
  exit 1
}

echo "== folders and the receive script"
nginx -t  # the existing config must be fine before we add anything
mkdir -p "$ROOT/releases" "$ROOT/acme"
sed "s/__DOMAIN__/$DOMAIN/g" "$SRC/receive.sh" > "$RECEIVE.new"
chmod 755 "$RECEIVE.new"
mv "$RECEIVE.new" "$RECEIVE"
install -m 644 "$SRC/nginx-drapewright-app.conf" "$SNIPPET"

if [ -e "$CERT" ]; then
  echo "== nginx site for $DOMAIN (HTTPS)"
  install_site "$SRC/nginx-drapewright.conf"
  echo "Server ready: https://$DOMAIN"
  exit 0
fi

echo "== nginx site for $DOMAIN (plain HTTP until the certificate exists)"
install_site "$SRC/nginx-drapewright-http.conf"

if ! points_here "$DOMAIN" || ! points_here "www.$DOMAIN"; then
  echo "== no HTTPS yet: DNS doesn't point here ($DOMAIN -> $(dns_of "$DOMAIN"), www.$DOMAIN -> $(dns_of "www.$DOMAIN"); this server is $(this_ip))."
  echo "   Point A records for both at $(this_ip) (and AAAA, if you add them, at $(this_ip6)); once they resolve, run ./deploy/deploy.sh again."
  exit 0
fi

echo "== HTTPS certificate for $DOMAIN and www.$DOMAIN"
command -v certbot >/dev/null || apt-get install -y --no-upgrade certbot
CERT_ARGS=(certonly --webroot -w "$ROOT/acme" --cert-name "$CERT_NAME" -d "$DOMAIN" -d "www.$DOMAIN"
  --deploy-hook "systemctl reload nginx" --non-interactive --agree-tos)
if [ -n "$EMAIL" ]; then CERT_ARGS+=(-m "$EMAIL"); else CERT_ARGS+=(--register-unsafely-without-email); fi
certbot "${CERT_ARGS[@]}"

echo "== nginx site for $DOMAIN (HTTPS)"
install_site "$SRC/nginx-drapewright.conf"
echo "Server ready: https://$DOMAIN"
