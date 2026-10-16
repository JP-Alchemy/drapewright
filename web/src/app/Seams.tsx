import type { DisplayUnits, Seam, SeamReport, SeamSide, Suggestion } from '@drapewright/engine'
import { useState } from 'react'
import { Button } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { sideLabel } from './labels.ts'

/** How a seam measures, in a line: its lengths and how its notches meet. */
export function seamSummary(report: SeamReport, units: DisplayUnits): string {
  if (report.problem) return t.seams.problem
  const length = (mm: number) => formatLength(mm, units)
  const parts: string[] = []
  const diff = report.difference
  if (Math.abs(diff) < 1) {
    parts.push(`${t.seams.sameLength}, ${length(report.aLength)}`)
  } else {
    parts.push(
      `${t.seams.lengths(length(report.aLength), length(report.bLength))}: ${t.seams.longer(
        diff > 0 ? 'second' : 'first',
        length(Math.abs(diff)),
      )}`,
    )
  }
  const apart = notchesApart(report)
  const alone = report.notches.filter((n) => n.apart === null).length
  if (apart !== null)
    parts.push(apart < 2 ? t.seams.notchesMeet : t.seams.notchesApart(length(apart)))
  if (alone > 0) parts.push(t.seams.notchAlone(alone))
  return parts.join('; ')
}

/** How far apart the notches that should meet on a seam are at most, mm; null with none. */
function notchesApart(report: SeamReport): number | null {
  const apart = report.notches.flatMap((n) => (n.apart === null ? [] : [Math.abs(n.apart)]))
  return apart.length > 0 ? Math.max(...apart) : null
}

/** Notches this far apart on a seam, mm, or further, are flagged: it may be the wrong way round. */
const NOTCHES_MISS = 10

/** What to check on a seam, if anything: an edge gone, or notches that miss. */
export function seamTrouble(report: SeamReport, units: DisplayUnits): string | null {
  if (report.problem) return t.seams.problem
  const apart = notchesApart(report)
  return apart !== null && apart >= NOTCHES_MISS
    ? t.seams.notchesMiss(formatLength(apart, units))
    : null
}

/** The same two sides, whichever way round. */
const sideKey = (s: SeamSide) =>
  `${s.piece}:${s.edge}:${s.mirror ? 1 : 0}:${s.from ?? ''}:${s.to ?? ''}`
const sidesKey = (seam: Seam) => [sideKey(seam.a), sideKey(seam.b)].sort().join('|')

/**
 * Finding seams: the edges that look sewn together, each shown joined on
 * the table when tapped, to sew or leave.
 */
export function FindSeams({ session }: { session: Session }) {
  const { suggestion: shown } = useStore(session.ui)
  useEngineState(session)
  const doc = session.engine.document()
  const [finding, setFinding] = useState(false)
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set())
  if (doc.pieces.length === 0) return null

  const suggestions = finding
    ? session.engine.suggestSeams().filter((s) => !dismissed.has(sidesKey(s.seam)))
    : []
  const shownKey = shown && sidesKey(shown)
  const sew = (chosen: Suggestion[]) => {
    const sewn = session.dispatch({
      type: 'addSeams',
      seams: chosen.map((s) => ({ ...s.seam, confirmed: true })),
    })
    if (sewn && chosen.some((s) => sidesKey(s.seam) === shownKey)) session.showSuggestion(null)
  }

  return (
    <div className="find-seams">
      {doc.seams.length === 0 && !finding && <p className="muted small">{t.seams.none}</p>}
      {!finding && doc.pieces.length > 1 && (
        <Button
          className="button"
          onPress={() => {
            // Panels drawn for one side are cut as pairs first, so their
            // centres are found too.
            session.cutOneSideAsPairs()
            setFinding(true)
          }}
        >
          {t.seams.find}
        </Button>
      )}
      {finding && (
        <div className="seam-suggestions" role="group" aria-label={t.seams.finding}>
          <h3 className="group-label">{t.seams.finding}</h3>
          {suggestions.length === 0 ? (
            <p className="muted small">{t.seams.noneFound}</p>
          ) : (
            <>
              <p className="muted small">{t.seams.findingHint}</p>
              <ul className="seam-list">
                {suggestions.map((s) => {
                  const key = sidesKey(s.seam)
                  const on = key === shownKey
                  return (
                    <li key={key} className="seam-item suggested" data-shown={on || undefined}>
                      <Button
                        className="seam-pick"
                        aria-pressed={on}
                        onPress={() => session.showSuggestion(on ? null : s.seam)}
                      >
                        <span className="seam-text">
                          <span className="piece-name">
                            {t.seams.between(sideLabel(doc, s.seam.a), sideLabel(doc, s.seam.b))}
                          </span>
                          <span className="muted small">
                            {[
                              t.seams.likely(s.confidence),
                              ...s.reasons.map((r) => t.seams.reasons[r]),
                            ].join(' · ')}
                          </span>
                        </span>
                      </Button>
                      <div className="seam-actions">
                        <Button className="button primary" onPress={() => sew([s])}>
                          {t.seams.sew}
                        </Button>
                        <Button
                          className="button"
                          onPress={() => {
                            if (on) session.showSuggestion(null)
                            setDismissed((d) => new Set(d).add(key))
                          }}
                        >
                          {t.seams.notThis}
                        </Button>
                      </div>
                    </li>
                  )
                })}
              </ul>
              <Button className="button" onPress={() => sew(suggestions)}>
                {t.seams.sewAll(suggestions.length)}
              </Button>
            </>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * The picked seam: the edges it sews and how they measure against each
 * other; turned round, or unpicked.
 */
export function SeamSection({ session, seam }: { session: Session; seam: Seam }) {
  const doc = session.engine.document()
  const i = doc.seams.findIndex((s) => s.id === seam.id)
  const report = session.engine.seamReports()[i]
  return (
    <section className="panel-section inspector" aria-labelledby="seam-heading">
      <h2 id="seam-heading">{t.seams.seam(i + 1, seam.name)}</h2>
      <p className="seam-sides">
        {t.seams.between(sideLabel(doc, seam.a), sideLabel(doc, seam.b))}
      </p>
      {report && <p className="muted small">{seamSummary(report, doc.settings.displayUnits)}</p>}
      <div className="button-row">
        <Button
          className="button"
          aria-description={t.seams.turnHint}
          onPress={() =>
            session.dispatch({ type: 'setSeam', seam: { ...seam, reversed: !seam.reversed } })
          }
        >
          {t.seams.turn}
        </Button>
        <Button
          className="button danger"
          onPress={() => session.dispatch({ type: 'deleteSeams', ids: [seam.id] })}
        >
          {t.seams.unpick}
        </Button>
      </div>
    </section>
  )
}
