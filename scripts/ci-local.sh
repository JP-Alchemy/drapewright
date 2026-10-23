#!/usr/bin/env bash
# CI on this computer (`pnpm ci:local`): what .github/workflows/ci.yml runs,
# in its order, then the drape tests in full Chromium drawing and draping
# with WebGPU (the chromium-webgpu project in web/playwright.config.ts),
# which CI's runners can't run, if this computer has it. Stops at the first
# step that fails, then lists each step's result and time.
#
# Exit codes are read straight from each command, never through a pipe:
# `pnpm -s` and `| tail` hide failures.

cd "$(dirname "$0")/.." || exit 1

if [ -t 1 ]; then
  bold=$'\e[1m' dim=$'\e[2m' red=$'\e[31m' green=$'\e[32m' off=$'\e[0m'
else
  bold='' dim='' red='' green='' off=''
fi

# The script decides when the WebGPU project runs: not in the whole suite.
unset E2E_WEBGPU

names=()
results=()
times=()
running=''
step_began=0

# Seconds as m:ss.
clock() { printf '%d:%02d' $(($1 / 60)) $(($1 % 60)); }

summary() {
  local status=$? i colour
  if [ -n "$running" ]; then
    names+=("$running")
    results+=('interrupted')
    times+=($((SECONDS - step_began)))
  fi
  local changes=''
  git diff --quiet HEAD 2>/dev/null || changes=', with uncommitted changes'
  echo
  echo "${bold}CI on this computer${off} ($(git rev-parse --short HEAD)$changes)"
  for i in "${!names[@]}"; do
    case ${results[$i]} in
      passed) colour=$green ;;
      skipped*) colour=$dim ;;
      *) colour=$red ;;
    esac
    printf '  %-34s %s%-44s%s %6s\n' "${names[$i]}" "$colour" "${results[$i]}" "$off" \
      "$(clock "${times[$i]}")"
  done
  printf '  %-34s %-44s %6s\n' 'Total' '' "$(clock "$SECONDS")"
  if [ "$status" -eq 0 ]; then
    echo "${green}${bold}All passed.${off}"
  else
    echo "${red}${bold}Stopped: ${names[${#names[@]} - 1]} ${results[${#results[@]} - 1]}.${off}"
  fi
}
trap summary EXIT
trap 'exit 130' INT TERM

# step NAME COMMAND...: runs COMMAND; if it fails, stops here.
step() {
  local name=$1 status
  shift
  echo
  echo "${bold}▶ $name${off}  ${dim}$*${off}"
  running=$name
  step_began=$SECONDS
  "$@"
  status=$?
  running=''
  names+=("$name")
  times+=($((SECONDS - step_began)))
  if [ $status -ne 0 ]; then
    results+=("failed (exit $status)")
    echo "${red}✗ $name failed (exit $status)${off}"
    exit $status
  fi
  results+=('passed')
}

# skip NAME WHY: a step not run, and why.
skip() {
  echo
  echo "${bold}▶ $1${off}  ${dim}skipped: $2${off}"
  names+=("$1")
  results+=("skipped: $2")
  times+=(0)
}

# As CI: the files `cargo test` writes are as committed (or staged).
generated_files_current() {
  git diff --exit-code -- packages/engine/src/generated docs/schema \
    crates/pattern-core/tests/gallery fixtures/drapewright && return
  echo "${red}These generated files differ from what is committed:${off} cargo test"
  echo "writes them, so commit (or stage) them with the change that made them."
  return 1
}

# Playwright reuses a server already listening on its ports (outside CI,
# reuseExistingServer), so another worktree's preview would quietly be
# tested instead of this build.
ports_free() {
  local port pid busy=0
  for port in 4173 4174; do
    if command -v lsof >/dev/null; then
      for pid in $(lsof -nP -t -iTCP:"$port" -sTCP:LISTEN 2>/dev/null); do
        busy=1
        echo "${red}Port $port is in use${off} by $(ps -o command= -p "$pid") (pid $pid),"
        echo "  run from $(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p')"
      done
    elif nc -z 127.0.0.1 "$port" 2>/dev/null || nc -z ::1 "$port" 2>/dev/null; then
      busy=1
      echo "${red}Port $port is in use.${off}"
    fi
  done
  if [ $busy -ne 0 ]; then
    echo "The e2e tests would test that server's build, not this one. Stop it, then run again."
    return 1
  fi
  echo "Ports 4173 and 4174 are free."
}

# The e2e suite as CI runs it (CI=true there forbids test.only, and retries
# a failed test once), on ports nothing else holds.
e2e() {
  ports_free || return 1
  pnpm e2e --forbid-only --retries=1 "$@"
}

# The drape tests in full Chromium with WebGPU: they fail if the drape isn't
# moved on the graphics card (tests/e2e/drape.spec.ts).
drape_with_webgpu() {
  E2E_WEBGPU=1 e2e --project=chromium-webgpu
}

# Rust (CI's rust job). CI also installs Mesa's software Vulkan for the
# drape on the graphics card's tests; here the computer's own card runs them.
step 'Rust formatting' cargo fmt --all --check
step 'Clippy' cargo clippy --workspace --all-targets -- -D warnings
step 'Clippy for the browser (wasm32)' cargo clippy -p drapewright-wasm-3d -p drapewright-wasm-core \
  --target wasm32-unknown-unknown -- -D warnings
step 'Rust tests' cargo test --workspace
step 'Generated files committed' generated_files_current

# Licences and advisories (CI's licences job).
if command -v cargo-deny >/dev/null; then
  step 'Licences and advisories (Rust)' cargo deny check
else
  skip 'Licences and advisories (Rust)' 'cargo install --locked cargo-deny'
fi

# The web (CI's web job).
step 'Dependencies' pnpm install --frozen-lockfile
step 'Licences (npm)' pnpm licences
step 'WebAssembly build' pnpm wasm
step 'WebAssembly size budget' scripts/wasm-size.sh
step 'Types' pnpm typecheck
step 'Lint' pnpm lint
step 'Formatting' pnpm format:check
step 'Unit tests' pnpm test
step 'Playwright browsers' pnpm --filter @drapewright/web exec playwright install chromium webkit
step 'End-to-end tests' e2e

# Beyond CI: the drape on the graphics card, as in most seamstresses'
# browsers.
echo
echo "${bold}Has Chromium WebGPU here?${off}"
(cd web && node tests/has-webgpu.mjs)
case $? in
  0) step 'Drape tests with WebGPU' drape_with_webgpu ;;
  3) skip 'Drape tests with WebGPU' 'no WebGPU adapter here' ;;
  *)
    names+=('Drape tests with WebGPU')
    results+=('failed: the WebGPU check failed')
    times+=(0)
    exit 1
    ;;
esac
