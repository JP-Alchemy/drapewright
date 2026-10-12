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
  const apart = report.notches.flatMap((n) => (n.apart === null ? [] : [Math.abs(n.apart)]))
  const alone = report.notches.filter((n) => n.apart === null).length
  if (apart.length > 0) {
    const most = Math.max(...apart)
    parts.push(most < 2 ? t.seams.notchesMeet : t.seams.notchesApart(length(most)))
  }
  if (alone > 0) parts.push(t.seams.notchAlone(alone))
  return parts.join('; ')
}

/** The same two sides, whichever way round. */
const sideKey = (s: SeamSide) =>
  `${s.piece}:${s.edge}:${s.mirror ? 1 : 0}:${s.from ?? ''}:${s.to ?? ''}`
const suggestionKey = (s: Suggestion) => [sideKey(s.seam.a), sideKey(s.seam.b)].sort().join('|')

/** The seams, numbered as on the table, and the suggestions to confirm. */
export function SeamList({ session }: { session: Session }) {
  const { seam: picked } = useStore(session.ui)
  useEngineState(session)
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const reports = session.engine.seamReports()
  const [finding, setFinding] = useState(false)
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set())
  if (doc.pieces.length === 0) return null

  const suggestions = finding
    ? session.engine.suggestSeams().filter((s) => !dismissed.has(suggestionKey(s)))
    : []
  const sew = (chosen: Suggestion[]) =>
    session.dispatch({
      type: 'addSeams',
      seams: chosen.map((s) => ({ ...s.seam, confirmed: true })),
    })
  const update = (seam: Seam) => session.dispatch({ type: 'setSeam', seam })

  return (
    <section className="panel-section" aria-labelledby="seams-heading">
      <h2 id="seams-heading">{t.seams.heading}</h2>
      {doc.seams.length === 0 && !finding && <p className="muted small">{t.seams.none}</p>}
      {doc.seams.length > 0 && (
        <ul className="seam-list">
          {doc.seams.map((seam, i) => {
            const report = reports[i]
            const open = picked === seam.id
            return (
              <li key={seam.id} className="seam-item" data-picked={open || undefined}>
                <Button
                  className="seam-pick"
                  aria-pressed={open}
                  onPress={() => session.selectSeam(open ? null : seam.id)}
                >
                  <span className="seam-badge" aria-hidden="true">
                    {i + 1}
                  </span>
                  <span className="seam-text">
                    <span className="piece-name">{t.seams.seam(i + 1, seam.name)}</span>
                    <span className="muted small">
                      {t.seams.between(sideLabel(doc, seam.a), sideLabel(doc, seam.b))}
                    </span>
                    {report && <span className="muted small">{seamSummary(report, units)}</span>}
                  </span>
                </Button>
                {open && (
                  <div className="seam-actions">
                    <Button
                      className="button"
                      aria-description={t.seams.turnHint}
                      onPress={() => update({ ...seam, reversed: !seam.reversed })}
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
                )}
              </li>
            )
          })}
        </ul>
      )}
      {!finding && doc.pieces.length > 1 && (
        <Button className="button" onPress={() => setFinding(true)}>
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
              <ul className="seam-list">
                {suggestions.map((s) => (
                  <li key={suggestionKey(s)} className="seam-item suggested">
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
                    <div className="seam-actions">
                      <Button className="button primary" onPress={() => sew([s])}>
                        {t.seams.sew}
                      </Button>
                      <Button
                        className="button"
                        onPress={() => setDismissed((d) => new Set(d).add(suggestionKey(s)))}
                      >
                        {t.seams.notThis}
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
              <Button className="button" onPress={() => sew(suggestions)}>
                {t.seams.sewAll(suggestions.length)}
              </Button>
            </>
          )}
        </div>
      )}
    </section>
  )
}
