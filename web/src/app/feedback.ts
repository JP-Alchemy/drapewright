// Feedback, by email: the app opens the person's own mail with a message
// started, saying which version of Drapewright and which browser; nothing
// is sent by the app itself. A pattern that didn't open right is reported
// the same way, with what was read from it (never the file itself, which
// the person attaches only if they may share it).

import { engineVersion, type ImportNote } from '@drapewright/engine'
import type { PendingImport } from '../state/session.ts'
import { t } from '../strings/en.ts'

export const FEEDBACK_EMAIL = 'juan.bothma@gmail.com'
export const LINKEDIN = 'https://www.linkedin.com/in/jp-bothma/'

/** A link opening a message to Drapewright, started. */
export function mailto(subject: string, body: string): string {
  const q = encodeURIComponent
  return `mailto:${FEEDBACK_EMAIL}?subject=${q(subject)}&body=${q(body)}`
}

/** Which Drapewright and which browser, for a report. */
export function aboutThisCopy(): string[] {
  const build = (import.meta.env.VITE_BUILD as string | undefined) ?? 'dev'
  return [t.feedback.version(build, engineVersion()), t.feedback.browser(navigator.userAgent)]
}

/** A note's kind, and its count or piece, as a line. */
function noteLine(note: ImportNote): string {
  const { kind, ...rest } = note
  const said = Object.entries(rest)
    .map(([k, v]) => `${k} ${String(v)}`)
    .join(', ')
  return said ? `${kind} (${said})` : kind
}

/**
 * What was read from a pattern file, to say it didn't open right: the
 * file's name, size and fingerprint, the pieces found and their sizes,
 * the scale, the sizes and pages, and what the importer noted.
 */
export function importReport(pending: PendingImport): string {
  const { result, fileName, bytes, pages } = pending
  const r = t.feedback.report
  const lines = [
    r.file(fileName, Math.round(bytes.length / 1024), result.sha256.slice(0, 16)),
    r.scale(result.scale.chosen.basis, result.scale.certain),
  ]
  if (result.pages.length > 0) {
    lines.push(
      r.pages(result.pages.filter((p) => p.leftOut === null).length, result.pages.length, pages),
    )
  }
  if (result.sizes.names.length > 0) lines.push(r.sizes(result.sizes.names))
  lines.push(r.pieces(result.pieces.length))
  for (const piece of result.pieces) {
    const xs = [
      piece.outline.start[0],
      ...piece.outline.edges.flatMap((e) => e.segments.map((s) => s.to[0])),
    ]
    const ys = [
      piece.outline.start[1],
      ...piece.outline.edges.flatMap((e) => e.segments.map((s) => s.to[1])),
    ]
    const w = Math.round(Math.max(...xs) - Math.min(...xs))
    const h = Math.round(Math.max(...ys) - Math.min(...ys))
    lines.push(r.piece(piece.name, w, h, piece.outline.edges.length, piece.cut[0]?.quantity ?? 1))
  }
  if (result.notes.length > 0) {
    lines.push(r.notes)
    for (const note of result.notes) lines.push(`  ${noteLine(note)}`)
  }
  return [...lines, '', ...aboutThisCopy()].join('\n')
}
