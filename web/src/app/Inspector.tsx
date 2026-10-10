import { useState } from 'react'
import { Input, Label, TextField } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'

/**
 * A text field that reports its value when editing ends, not per keystroke.
 * Give it a `key` that changes with `value` so outside changes (undo) show.
 */
function CommitTextField({
  label,
  value,
  onCommit,
}: {
  label: string
  value: string
  onCommit(v: string): void
}) {
  const [draft, setDraft] = useState(value)
  const commit = () => {
    const next = draft.trim()
    if (next && next !== value) onCommit(next)
    else setDraft(value)
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
      <Input className="text-input" />
    </TextField>
  )
}

/** Details of the selected piece and edge, editable in sewing terms. */
export function Inspector({ session }: { session: Session }) {
  const { selection, edge } = useStore(session.ui)
  useEngineState(session)
  const units = session.engine.document().settings.displayUnits

  if (selection.size === 0) {
    return (
      <section className="panel-section">
        <p className="muted small">{t.inspector.nothing}</p>
      </section>
    )
  }
  if (selection.size > 1) {
    return (
      <section className="panel-section">
        <p className="muted small">{t.inspector.many(selection.size)}</p>
      </section>
    )
  }
  const id = [...selection][0]!
  const sp = session.scenePiece(id)
  if (!sp) return null
  const { piece, view } = sp
  const [x0, y0, x1, y1] = view.outlineBounds
  const selectedEdge =
    edge?.piece === piece.id ? view.edges.find((e) => e.id === edge.edge) : undefined

  return (
    <section className="panel-section inspector" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">{t.inspector.heading}</h2>
      <CommitTextField
        key={`${piece.id}:${piece.name}`}
        label={t.inspector.name}
        value={piece.name}
        onCommit={(name) => session.dispatch({ type: 'renamePiece', piece: piece.id, name })}
      />
      <p className="muted small">
        {t.pieces.size(formatLength(x1 - x0, units), formatLength(y1 - y0, units))}
      </p>
      <LengthField
        label={t.inspector.seamAllowance}
        value={piece.allowance.seam}
        units={units}
        onChange={(seam) =>
          session.dispatch(
            { type: 'setAllowance', piece: piece.id, allowance: { ...piece.allowance, seam } },
            // Stepping up and down is one undo step.
            { coalesce: `allowance:${piece.id}` },
          )
        }
      />
      {selectedEdge && (
        <div className="edge-details">
          <h3>{selectedEdge.name || t.inspector.edge}</h3>
          <p className="muted small">
            {t.inspector.edgeLength(formatLength(selectedEdge.length, units))}
          </p>
        </div>
      )}
    </section>
  )
}
