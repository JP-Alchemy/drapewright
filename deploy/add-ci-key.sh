#!/usr/bin/env bash
# One-time setup for GitHub Actions deploys (run from the repo, on your machine,
# after the first ./deploy/deploy.sh).
#
#   ./deploy/add-ci-key.sh        (SERVER defaults to root@91.98.193.172)
#
# 1. Makes a new SSH key just for CI (~/.ssh/drapewright-ci); your own key isn't shared.
# 2. Lets that key run ONLY /usr/local/bin/drapewright-receive on the server
#    (forced command + "restrict": no shell, no forwarding, nothing else).
# 3. Saves the key and the server's host fingerprint as GitHub secrets of this
#    repo (DEPLOY_SSH_KEY, DEPLOY_KNOWN_HOSTS) with the gh CLI if you have it,
#    otherwise tells you what to paste where.
# Run it again to replace the key; delete the "drapewright-ci" line in
# /root/.ssh/authorized_keys on the server to revoke it.
set -euo pipefail
SERVER="${SERVER:-root@91.98.193.172}"
HOST="${SERVER#*@}"
KEY="${KEY:-$HOME/.ssh/drapewright-ci}"
cd "$(dirname "$0")/.."

ssh "$SERVER" 'test -x /usr/local/bin/drapewright-receive' ||
  { echo "Run ./deploy/deploy.sh first: it installs the receive script on the server." >&2; exit 1; }

[ -f "$KEY" ] || ssh-keygen -t ed25519 -N '' -C drapewright-ci -f "$KEY"
LINE="command=\"/usr/local/bin/drapewright-receive\",restrict $(cat "$KEY.pub")"

echo "== installing the CI key on $HOST (it can only run drapewright-receive)"
ssh "$SERVER" "mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys \
  && sed -i '/ drapewright-ci\$/d' ~/.ssh/authorized_keys && printf '%s\n' '$LINE' >> ~/.ssh/authorized_keys"

KNOWN=$(ssh-keyscan -t ed25519,ecdsa,rsa "$HOST" 2>/dev/null)
[ -n "$KNOWN" ] || { echo "Couldn't read the server's host keys" >&2; exit 1; }

if command -v gh >/dev/null && gh auth status >/dev/null 2>&1; then
  gh secret set DEPLOY_SSH_KEY < "$KEY"
  printf '%s\n' "$KNOWN" | gh secret set DEPLOY_KNOWN_HOSTS
  gh variable set DEPLOY_HOST --body "$HOST"
  echo "== GitHub secrets set. Pushes to the production branch now deploy (after the checks pass)."
else
  echo
  echo "== Add these in GitHub: repo → Settings → Secrets and variables → Actions"
  echo "   Secret DEPLOY_SSH_KEY:     the whole contents of $KEY  (e.g. cat $KEY | pbcopy)"
  echo "   Secret DEPLOY_KNOWN_HOSTS: these lines:"
  printf '%s\n' "$KNOWN" | sed 's/^/     /'
  echo "   Variable DEPLOY_HOST:      $HOST"
fi
