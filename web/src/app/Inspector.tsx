import type { EdgeKind, Piece, PieceView } from '@drapewright/engine'
import { useState } from 'react'
import { Button, Input, Label, Radio, RadioGroup, Switch, TextField } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { PartRef, Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { CornerChooser } from './CornerChooser.tsx'
import { LengthField } from './LengthField.tsx'
import { AlterSection } from './AlterSection.tsx'
import { CutSection, GrainSection, NotchSection } from './PieceDetails.tsx'
import { edgeLabel, sideLabel } from './labels.ts'

/**
 * A text field that reports its value when editing ends, not per keystroke.
 * Give it a `key` that changes with `value` so outside changes (undo) show.
 */
export function CommitTextField({
  label,
  value,
  placeholder,
  onCommit,
}: {
  label: string
  value: string
  placeholder?: string
  onCommit(v: string): void
}) {
  const [draft, setDraft] = useState(value)
  const commit = () => {
    const next = draft.trim()
    if (next !== value) onCommit(next)
  }
  return (
    <TextField
      className="field"
      value={draft}
      onChange={setDraft}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    >
      <Label>{label}</Label>
      <Input className="text-input" placeholder={placeholder} />
    </TextField>
  )
}

function PieceSection({
  session,
  piece,
  view,
}: {
  session: Session
  piece: Piece
  view: PieceView
}) {
  const units = session.engine.document().settings.displayUnits
  const [x0, y0, x1, y1] = view.outlineBounds
  const included = piece.allowance.mode === 'included'
  const setAllowance = (allowance: Piece['allowance']) =>
    session.dispatch(
      { type: 'setAllowance', piece: piece.id, allowance },
      // Stepping up and down is one undo step.
      { coalesce: `allowance:${piece.id}` },
    )

  return (
    <section className="panel-section inspector" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">{t.inspector.heading}</h2>
      <CommitTextField
        key={`${piece.id}:${piece.name}`}
        label={t.inspector.name}
        value={piece.name}
        onCommit={(name) =>
          name && session.dispatch({ type: 'renamePiece', piece: piece.id, name })
        }
      />
      <p className="muted small">
        {t.pieces.size(formatLength(x1 - x0, units), formatLength(y1 - y0, units))}
      </p>
      <Switch
        className="switch"
        isSelected={included}
        onChange={(on) => setAllowance({ ...piece.allowance, mode: on ? 'included' : 'add' })}
      >
        <span className="switch-track" aria-hidden="true" />
        {t.inspector.included}
      </Switch>
      <p className="muted small">{included ? t.inspector.includedHint : t.inspector.addedHint}</p>
      <LengthField
        label={t.inspector.seamAllowance}
        value={piece.allowance.seam}
        units={units}
        onChange={(seam) => setAllowance({ ...piece.allowance, seam })}
      />
      <LengthField
        label={t.inspector.hemAllowance}
        value={piece.allowance.hem ?? piece.allowance.seam}
        units={units}
        onChange={(hem) => setAllowance({ ...piece.allowance, hem })}
      />
      <GrainSection session={session} piece={piece} view={view} />
      <CutSection session={session} piece={piece} />
      <AlterSection session={session} piece={piece} view={view} />
      {view.allowanceNotes.length + view.issues.length > 0 && (
        <ul className="notes">
          {view.allowanceNotes.map((n, i) => (
            <li key={`n${i}`}>{t.allowanceNote(n, (id) => edgeLabel(piece, id))}</li>
          ))}
          {view.issues.map((issue, i) => (
            <li key={`i${i}`}>{issue.message}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

function EdgeSection({
  session,
  piece,
  view,
  edgeId,
}: {
  session: Session
  piece: Piece
  view: PieceView
  edgeId: string
}) {
  const units = session.engine.document().settings.displayUnits
  const edge = piece.outline.edges.find((e) => e.id === edgeId)
  const ev = view.edges.find((e) => e.id === edgeId)
  if (!edge || !ev) return null
  const ref = { piece: piece.id, edge: edgeId }
  const doc = session.engine.document()
  const here = (side: { piece: string; edge: string }) =>
    side.piece === piece.id && side.edge === edgeId
  const sewn = doc.seams
    .map((seam, i) => ({ seam, i }))
    .filter(({ seam }) => here(seam.a) || here(seam.b))
  return (
    <section className="panel-section inspector" aria-labelledby="edge-heading">
      <h2 id="edge-heading">{t.inspector.edge}</h2>
      {sewn.map(({ seam, i }) => (
        <Button key={seam.id} className="link-button" onPress={() => session.selectSeam(seam.id)}>
          {t.seams.sewnTo(sideLabel(doc, here(seam.a) && !seam.a.mirror ? seam.b : seam.a))} (
          {t.seams.seam(i + 1, seam.name)})
        </Button>
      ))}
      <Button className="button" onPress={() => session.startSewing(piece.id, edgeId)}>
        {t.seams.sewTo}
      </Button>
      <CommitTextField
        key={`${edgeId}:${edge.name}`}
        label={t.inspector.edgeName}
        value={edge.name}
        placeholder={t.inspector.edgeNamePlaceholder}
        onCommit={(name) => session.dispatch({ type: 'renameEdge', ...ref, name })}
      />
      <p className="muted small">{t.inspector.edgeLength(formatLength(ev.length, units))}</p>
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        value={edge.kind}
        onChange={(kind) =>
          session.dispatch({ type: 'setEdgeKind', ...ref, kind: kind as EdgeKind })
        }
      >
        <Label className="group-label">{t.inspector.kind}</Label>
        <div className="segments">
          {(['seam', 'hem', 'fold'] as const).map((k) => (
            <Radio key={k} value={k} className="segment">
              {t.inspector.kinds[k]}
            </Radio>
          ))}
        </div>
        <p className="muted small">{t.inspector.kindHints[edge.kind]}</p>
      </RadioGroup>
      {edge.kind === 'fold' ? (
        <p className="muted small">{t.inspector.foldNoAllowance}</p>
      ) : (
        <>
          <LengthField
            label={t.inspector.edgeAllowance}
            value={ev.allowance}
            units={units}
            onChange={(allowance) =>
              session.dispatch(
                { type: 'setEdgeAllowance', ...ref, allowance },
                { coalesce: `edge-allowance:${edgeId}` },
              )
            }
          />
          {edge.allowance !== null && (
            <Button
              className="link-button"
              onPress={() =>
                session.dispatch({ type: 'setEdgeAllowance', ...ref, allowance: null })
              }
            >
              {t.inspector.usePieceAllowance}
            </Button>
          )}
        </>
      )}
    </section>
  )
}

function CornerSection({
  session,
  piece,
  edgeId,
}: {
  session: Session
  piece: Piece
  edgeId: string
}) {
  const i = piece.outline.edges.findIndex((e) => e.id === edgeId)
  if (i < 0) return null
  const next = piece.outline.edges[(i + 1) % piece.outline.edges.length]!
  return (
    <section className="panel-section inspector" aria-labelledby="corner-heading">
      <h2 id="corner-heading">{t.corner.sectionHeading}</h2>
      <p className="muted small">
        {t.corner.between(edgeLabel(piece, edgeId), edgeLabel(piece, next.id))}
      </p>
      <CornerChooser
        session={session}
        piece={piece}
        edgeId={edgeId}
        left={edgeLabel(piece, edgeId)}
        right={edgeLabel(piece, next.id)}
      />
      {piece.outline.edges.length > 1 && (
        <Button
          className="button"
          onPress={() => session.dispatch({ type: 'joinEdges', piece: piece.id, edge: edgeId })}
        >
          {t.corner.remove}
        </Button>
      )}
    </section>
  )
}

/** Details of the selected piece, edge or corner, editable in sewing terms. */
export function Inspector({ session }: { session: Session }) {
  const { selection, part } = useStore(session.ui)
  useEngineState(session)

  if (selection.size !== 1) {
    return (
      <section className="panel-section">
        <p className="muted small">
          {selection.size === 0 ? t.inspector.nothing : t.inspector.many(selection.size)}
        </p>
      </section>
    )
  }
  const sp = session.scenePiece([...selection][0]!)
  if (!sp) return null
  const { piece, view } = sp
  const own: PartRef | null = part?.piece === piece.id ? part : null
  return (
    <>
      {own?.kind === 'corner' && (
        <CornerSection session={session} piece={piece} edgeId={own.edge} />
      )}
      {own?.kind === 'edge' && (
        <EdgeSection session={session} piece={piece} view={view} edgeId={own.edge} />
      )}
      {own?.kind === 'notch' && (
        <NotchSection session={session} piece={piece} view={view} notchId={own.notch} />
      )}
      <PieceSection session={session} piece={piece} view={view} />
    </>
  )
}
