# Drapewright

An open-source garment pattern studio for home sewists that
runs in the browser. Open the patterns you already own, fit them to your
measurements, see them draped in 3D, adjust them, and print them at exactly
the right size.

**Status:** Phase 1 (true-scale foundation) is complete for review: open
SVG and DXF patterns, view them at true size, set seam allowance edge by
edge, mark notches and grainlines, and print a tiled PDF at exactly 100 %.
Projects are saved on the device and can be backed up as `.drapewright` files.
Phase 2 has started: PDF patterns open too, copy-shop sheets and home-print
pages alike (tiles are put back together), and AI assistants can work on
projects through an MCP server.

## Documents

* [Engineering proposal](docs/proposal.md): architecture, layout, input
  design, Phase 1 plan
* [Project file format](docs/file-format.md): the open `.drapewright` format, with
  its [JSON Schema](docs/schema/project.schema.json)
* [Open questions](docs/open-questions.md): sewing and product decisions
  waiting for an answer
* [Licences](docs/licences.md): every dependency and asset
* [Demo notes](docs/demo-notes.md): what you can try after each step
* [Working with an AI assistant](docs/ai-assistants.md): the MCP server,
  so assistants such as Claude can open, check and change projects

## Development

Requirements: Rust (the toolchain in `rust-toolchain.toml` installs
itself), `wasm-pack`, Node 22+, pnpm 9+.

```bash
pnpm install
pnpm wasm      # build the Rust engine to WebAssembly: the 2D engine (shrunk with wasm-opt) and the 3D side
pnpm dev       # http://localhost:5173 (the site; the app is at /app/, served cross-origin isolated)
```

Tests:

```bash
cargo test --workspace   # Rust: unit, property and golden tests
pnpm test                # TypeScript unit tests
pnpm e2e                 # Playwright, Chromium + WebKit
```

## AI assistants (MCP)

`drapewright-mcp` lets an AI assistant work on Drapewright projects on your
computer: import patterns, look at the pieces, name them, set allowances,
cutting, notches and grainlines, and save a project to open in the app.

```bash
cargo build --release -p drapewright-mcp
claude mcp add drapewright -- "$PWD/target/release/drapewright-mcp"
```

Details and other assistants: [docs/ai-assistants.md](docs/ai-assistants.md).

## Hosting

Drapewright is a static site: nginx on our own server (Hetzner) serves it at
**https://drapewright.com**, with the headers the app needs (cross-origin
isolation, a strict content policy). `www.drapewright.com` redirects to it;
browsers keep saved projects per address, so the app has exactly one. The
setup shares the server with other sites without touching them:

- **nginx:** it adds one site file (`zz-drapewright.conf`) and one snippet,
  answering only for its own names.
  - The file loads after the other sites, so it never becomes nginx's default
    site.
  - The config is tested first. If nginx rejects the new file, or the default
    site would change, the file is removed again without reloading.
  - Reloads are graceful.
- **certbot:** a certificate named `drapewright`, for `drapewright.com` and
  `www.drapewright.com` only. It is obtained through a webroot, so certbot
  never edits nginx files. Renewals (the server's certbot timer) reload nginx.
- **Releases:** each one goes in its own folder in `/var/www/drapewright/releases`.
  - The site switches to a new release in one step and checks it is served,
    switching back if not.
  - The five newest releases are kept.
  - `https://drapewright.com/version.txt` shows which commit is live.

1. **DNS** (Namecheap → Domain List → drapewright.com → Advanced DNS):
   - **Delete** the parking records (`CNAME www → parkingpage.namecheap.com`
     and the `@` URL redirect).
   - **Add** these records:

     | Type | Host | Value |
     |---|---|---|
     | A | `@` | `91.98.193.172` |
     | A | `www` | `91.98.193.172` |
     | AAAA *(optional)* | `@` | `2a01:4f8:c0c:bfbb::1` |
     | AAAA *(optional)* | `www` | `2a01:4f8:c0c:bfbb::1` |
2. **Check first** (changes nothing):
   ```bash
   CHECK=1 ./deploy/deploy.sh
   ```
3. **Deploy** from your machine, in the repo:
   ```bash
   ./deploy/deploy.sh   # defaults: SERVER=root@91.98.193.172 DOMAIN=drapewright.com
   ```
   - It builds, sets the server up if needed (`deploy/setup-server.sh`), and
     ships the release.
   - Until DNS points at the server the site runs on plain HTTP. Once it
     does, the next run gets the certificate and switches to HTTPS.
   - `EMAIL=you@example.com` gets certificate notices; `SETUP=1` redoes the
     server setup.
4. **To remove it:** `./deploy/uninstall.sh` removes the nginx site. Add
   `PURGE=1` to also delete the releases, the receive script, the CI key and
   the certificate.

### Automatic deploys (GitHub Actions)

`.github/workflows/ci.yml` deploys when the **`production`** branch changes,
after every check has passed (Rust, licences, web with end-to-end tests). It
ships the exact files the checks built. Work happens on `main`; to release:
```bash
git push origin main:production
```
To roll back, point `production` at an earlier commit:
```bash
git push --force origin <commit>:production
```

**The server side is locked down:**
- The CI key can only run `/usr/local/bin/drapewright-receive`
  (`deploy/receive.sh`, forced in `authorized_keys` with `restrict`). It can't
  open a shell or forward ports.
- That script accepts only the app's files (no other paths, links or `../`).

**One-time setup** (after the first `./deploy/deploy.sh`), from your machine:
```bash
./deploy/add-ci-key.sh
```
- It makes a key just for CI (`~/.ssh/drapewright-ci`) and installs it on the
  server, restricted to the receive script.
- It saves the GitHub secrets `DEPLOY_SSH_KEY` and `DEPLOY_KNOWN_HOSTS` (and
  the variable `DEPLOY_HOST`) with the `gh` CLI, or tells you what to paste.
- Until the secrets exist, the deploy job skips with a warning.
- **To revoke:** delete the `drapewright-ci` line in `/root/.ssh/authorized_keys`
  on the server.

## Licence

Not chosen yet; see [open questions](docs/open-questions.md#licence).
