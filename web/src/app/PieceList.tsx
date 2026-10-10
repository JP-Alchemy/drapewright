import { ListBox, ListBoxItem } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'

export function PieceList({ session }: { session: Session }) {
  const { selection } = useStore(session.ui)
  useEngineState(session)
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const scene = session.scene()

  return (
    <section className="panel-section" aria-labelledby="pieces-heading">
      <h2 id="pieces-heading">{t.pieces.heading}</h2>
      {scene.length === 0 ? (
        <p className="muted small">{t.pieces.none}</p>
      ) : (
        <ListBox
          aria-labelledby="pieces-heading"
          className="piece-list"
          selectionMode="multiple"
          selectionBehavior="replace"
          selectedKeys={selection}
          onSelectionChange={(keys) =>
            session.select(keys === 'all' ? scene.map((s) => s.piece.id) : [...keys].map(String))
          }
          items={scene.map((s) => ({ id: s.piece.id, ...s }))}
        >
          {({ piece, view }) => {
            const [x0, y0, x1, y1] = view.outlineBounds
            return (
              <ListBoxItem id={piece.id} textValue={piece.name} className="piece-item">
                <span className="piece-name">{piece.name}</span>
                <span className="muted small">
                  {t.pieces.size(formatLength(x1 - x0, units), formatLength(y1 - y0, units))}
                </span>
              </ListBoxItem>
            )
          }}
        </ListBox>
      )}
    </section>
  )
}
