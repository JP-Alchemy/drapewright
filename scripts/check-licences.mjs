// Fails if a production npm package has a licence outside the allow-list
// in docs/licences.md. Rust crates are checked by cargo-deny (deny.toml).
import { execFileSync } from 'node:child_process'

const ALLOWED = new Set([
  'MIT',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'Zlib',
  'Unicode-3.0',
  'CC0-1.0',
  '0BSD',
  'BSL-1.0',
])

/** An SPDX expression such as "(MIT OR Apache-2.0)". */
function allowed(expression) {
  const e = expression.replace(/[()]/g, '').trim()
  if (e.includes(' AND ')) return e.split(' AND ').every(allowed)
  if (e.includes(' OR ')) return e.split(' OR ').some(allowed)
  return ALLOWED.has(e)
}

const byLicence = JSON.parse(
  execFileSync('pnpm', ['licenses', 'list', '--prod', '--json'], { encoding: 'utf8' }),
)
const refused = Object.entries(byLicence)
  .filter(([licence]) => !allowed(licence))
  .map(([licence, pkgs]) => `${licence}: ${[...new Set(pkgs.map((p) => p.name))].join(', ')}`)
const count = new Set(Object.values(byLicence).flatMap((pkgs) => pkgs.map((p) => p.name))).size

if (refused.length > 0) {
  console.error(`Licences outside the allow-list:\n  ${refused.join('\n  ')}`)
  process.exit(1)
}
console.log(`${count} production npm packages, all on the licence allow-list.`)
