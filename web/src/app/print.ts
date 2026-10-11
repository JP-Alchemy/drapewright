// What the printed PDF says, built from the strings so the words stay in
// one place; the engine draws everything else.

import type { Document, Paper, PdfTexts, PrintPlan } from '@drapewright/engine'
import { t } from '../strings/en.ts'
import { cutSummary } from './PieceDetails.tsx'

export function paperLabel(paper: Paper): string {
  return typeof paper === 'string' ? t.print.papers[paper] : t.print.custom
}

export function pdfTexts(doc: Document, plan: PrintPlan): PdfTexts {
  const pages = plan.pages.filter((p) => p.printed).length
  const joining = doc.print.assembly === 'trim' ? t.print.trim : t.print.overlap
  const instructions = [
    t.print.pdf.scale,
    ...(doc.print.testSquare ? [t.print.pdf.measure] : []),
    doc.print.assembly === 'trim' ? t.print.pdf.trim : t.print.pdf.overlap,
    ...(plan.pages.some((p) => !p.printed) ? [t.print.pdf.skipped] : []),
  ]
  return {
    title: doc.meta.name,
    subtitle: t.print.pdf.subtitle(pages, paperLabel(doc.print.paper), joining),
    instructions,
    squareMetric: t.print.pdf.squareMetric,
    squareImperial: t.print.pdf.squareImperial,
    pageMap: t.print.pdf.pageMap,
    notPrinted: t.print.pdf.notPrinted,
    placeOnFold: t.print.pdf.placeOnFold,
    pieceLines: Object.fromEntries(doc.pieces.map((p) => [p.id, [cutSummary(p)].filter(Boolean)])),
  }
}
