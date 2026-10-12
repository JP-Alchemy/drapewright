import { Circle, CircleCheck } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button, ListBox, ListBoxItem } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { cutShort } from './PieceDetails.tsx'

/**
 * The pieces, each with its size and how many to cut. Tapping one chooses
 * it; Select turns tapping into ticking, to choose several at once (as
 * lists on a tablet do) until Done. With a keyboard, Shift and Command
 * choose several too.
 */
export function PieceList({ session }: { session: Session }) {
  const { selection } = useStore(session.ui)
  const [ticking, setTicking] = useState(false)
  // How the list was last pressed: a finger's tap ticks (there are no
  // modifier keys to choose several), so unless ticking it chooses one.
  const pointer = useRef('')
  // A piece chosen on the table is shown in the list, as the list scrolls
  // on its own.
  const list = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (selection.size !== 1) return
    const [id] = selection
    list.current
      ?.querySelector(`[data-piece="${CSS.escape(id!)}"]`)
      ?.closest('[role="option"]')
      ?.scrollIntoView({ block: 'nearest' })
  }, [selection])
  useEngineState(session)
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const scene = session.scene()

  return (
    <section className="panel-section" aria-labelledby="pieces-heading">
      <div className="section-head">
        <h2 id="pieces-heading">{t.pieces.heading}</h2>
        {scene.length > 1 &&
          (ticking ? (
            <span className="head-actions">
              <Button
                className="link-button"
                onPress={() => session.select(scene.map((s) => s.piece.id))}
              >
                {t.pieces.selectAll}
              </Button>
              <Button className="link-button" onPress={() => setTicking(false)}>
                {t.pieces.done}
              </Button>
            </span>
          ) : (
            <Button
              className="link-button"
              aria-label={t.pieces.selectSeveral}
              onPress={() => setTicking(true)}
            >
              {t.pieces.select}
            </Button>
          ))}
      </div>
      {scene.length === 0 ? (
        <p className="muted small">{t.pieces.none}</p>
      ) : (
        <div ref={list} onPointerDownCapture={(e) => (pointer.current = e.pointerType)}>
          <ListBox
            aria-labelledby="pieces-heading"
            className="piece-list"
            selectionMode="multiple"
            selectionBehavior={ticking ? 'toggle' : 'replace'}
            selectedKeys={selection}
            onSelectionChange={(keys) => {
              const next = keys === 'all' ? scene.map((s) => s.piece.id) : [...keys].map(String)
              const tapped = [
                ...next.filter((id) => !selection.has(id)),
                ...[...selection].filter((id) => !next.includes(id)),
              ]
              if (!ticking && pointer.current === 'touch' && tapped.length === 1) {
                session.select(tapped)
              } else {
                session.select(next)
              }
            }}
            items={scene.map((s) => ({ id: s.piece.id, ...s }))}
          >
            {({ piece, view }) => {
              const [x0, y0, x1, y1] = view.outlineBounds
              return (
                <ListBoxItem
                  id={piece.id}
                  textValue={piece.name}
                  className={ticking ? 'piece-item ticking' : 'piece-item'}
                >
                  {({ isSelected }) => (
                    <>
                      {ticking &&
                        (isSelected ? (
                          <CircleCheck className="tick" size={22} aria-hidden="true" />
                        ) : (
                          <Circle className="tick" size={22} aria-hidden="true" />
                        ))}
                      <span className="piece-name" data-piece={piece.id}>
                        {piece.name}
                      </span>
                      <span className="muted small">
                        {t.pieces.size(formatLength(x1 - x0, units), formatLength(y1 - y0, units))}{' '}
                        · {cutShort(piece)}
                      </span>
                    </>
                  )}
                </ListBoxItem>
              )
            }}
          </ListBox>
        </div>
      )}
    </section>
  )
}
