import type { Seam } from '@drapewright/engine'
import { ChevronRight, Circle, CircleCheck, TriangleAlert } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button, Tree, TreeItem, TreeItemContent } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { cutShort } from './PieceDetails.tsx'
import { FindSeams, seamTrouble } from './Seams.tsx'
import { seamUnder } from './labels.ts'

/** A seam as listed under one of its pieces, numbered as on the table. */
interface SeamRow {
  key: string
  seam: Seam
  number: number
  piece: string
}

/**
 * The pieces, each with its size and how many to cut, and under it the
 * seams that sew it to the others. Tapping a piece chooses it, and opens it
 * to show its seams; tapping a seam picks it, joining its two edges on the
 * table. Select turns tapping into ticking, to choose several pieces at
 * once (as lists on a tablet do) until Done. With a keyboard, Shift and
 * Command choose several too.
 */
export function PieceList({ session }: { session: Session }) {
  const { selection, seam: picked } = useStore(session.ui)
  const [ticking, setTicking] = useState(false)
  // How the list was last pressed: a finger's tap ticks (there are no
  // modifier keys to choose several), so unless ticking it chooses one.
  const pointer = useRef('')
  // A piece chosen on its own opens to show its seams, and stays open (so
  // nothing moves under the pointer) until closed.
  const only = selection.size === 1 ? [...selection][0]! : null
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set(only ? [only] : []))
  const [openedFor, setOpenedFor] = useState(only)
  if (only !== openedFor) {
    setOpenedFor(only)
    if (only !== null && !open.has(only)) setOpen(new Set(open).add(only))
  }
  // What is chosen elsewhere is shown in the list, as the list scrolls on
  // its own: the seam picked, or the piece chosen and as many of its seams
  // as fit (not when chosen in the list: the list stays still under it).
  const list = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (only === null) return
    const all = (selector: string) => [...(list.current?.querySelectorAll(selector) ?? [])]
    const id = CSS.escape(only)
    const shown = picked
      ? all(`[data-seam="${CSS.escape(picked)}"][data-under="${id}"]`)
      : all(`[data-piece="${id}"]`)
    if (!picked && !list.current?.contains(document.activeElement)) {
      shown.unshift(...all(`[data-under="${id}"]`).slice(-1))
    }
    for (const row of shown) row.scrollIntoView({ block: 'nearest' })
  }, [only, picked])
  useEngineState(session)
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const scene = session.scene()
  const reports = session.engine.seamReports()

  // Each seam under each piece it sews: under one, sewing a piece to itself.
  const under = new Map<string, SeamRow[]>()
  doc.seams.forEach((seam, i) => {
    for (const piece of new Set([seam.a.piece, seam.b.piece])) {
      const row = { key: `seam:${seam.id}:${piece}`, seam, number: i + 1, piece }
      under.set(piece, [...(under.get(piece) ?? []), row])
    }
  })
  const rows = new Map([...under.values()].flat().map((row) => [row.key, row]))
  const pickedRows = ticking ? [] : [...rows.values()].filter((row) => row.seam.id === picked)
  const chosen = new Set([...selection, ...pickedRows.map((row) => row.key)])

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
        <div
          ref={list}
          onPointerDownCapture={(e) => (pointer.current = e.pointerType)}
          onKeyDownCapture={() => (pointer.current = 'keyboard')}
        >
          <Tree
            aria-labelledby="pieces-heading"
            className="piece-tree"
            selectionMode="multiple"
            selectionBehavior={ticking ? 'toggle' : 'replace'}
            selectedKeys={chosen}
            expandedKeys={open}
            onExpandedChange={(keys) => setOpen(new Set([...keys].map(String)))}
            onSelectionChange={(keys) => {
              const next = keys === 'all' ? scene.map((s) => s.piece.id) : [...keys].map(String)
              const added = next.filter((key) => !chosen.has(key))
              const removed = [...chosen].filter((key) => !next.includes(key))
              // A seam tapped, or moved onto with the keyboard: picked, on
              // the piece it is under. Clicked again, it is put down; a
              // finger's second tap unticks it, which puts it down too.
              const [one] = added.length === 1 ? added : next.length === 1 ? next : []
              const row = rows.get(one ?? '')
              if (row) {
                const again =
                  added.length === 0 &&
                  row.seam.id === picked &&
                  selection.has(row.piece) &&
                  pointer.current === 'mouse'
                session.selectSeam(again ? null : row.seam.id, row.piece)
                return
              }
              if (added.length === 0 && removed.length > 0 && removed.every((k) => rows.has(k))) {
                session.selectSeam(null)
                return
              }
              const pieces = next.filter((key) => !rows.has(key))
              const tapped = [
                ...pieces.filter((id) => !selection.has(id)),
                ...[...selection].filter((id) => !pieces.includes(id)),
              ]
              if (!ticking && pointer.current === 'touch' && tapped.length === 1) {
                session.select(tapped)
              } else {
                session.select(pieces)
              }
            }}
          >
            {scene.map(({ piece, view }) => {
              const [x0, y0, x1, y1] = view.outlineBounds
              return (
                <TreeItem
                  key={piece.id}
                  id={piece.id}
                  textValue={piece.name}
                  className={ticking ? 'piece-item ticking' : 'piece-item'}
                  data-piece={piece.id}
                >
                  <TreeItemContent>
                    {({ isSelected, hasChildItems }) => (
                      <>
                        {ticking ? (
                          isSelected ? (
                            <CircleCheck className="tick" size={22} aria-hidden="true" />
                          ) : (
                            <Circle className="tick" size={22} aria-hidden="true" />
                          )
                        ) : hasChildItems ? (
                          <Button slot="chevron" className="tree-chevron">
                            <ChevronRight size={18} aria-hidden="true" />
                          </Button>
                        ) : (
                          <span className="tree-chevron" aria-hidden="true" />
                        )}
                        <span className="piece-name">{piece.name}</span>
                        <span className="muted small">
                          {t.pieces.size(
                            formatLength(x1 - x0, units),
                            formatLength(y1 - y0, units),
                          )}{' '}
                          · {cutShort(piece)}
                        </span>
                      </>
                    )}
                  </TreeItemContent>
                  {!ticking &&
                    (under.get(piece.id) ?? []).map((row) => {
                      const { to, edges } = seamUnder(doc, piece.id, row.seam)
                      const report = reports[row.number - 1]
                      const trouble = report ? seamTrouble(report, units) : null
                      return (
                        <TreeItem
                          key={row.key}
                          id={row.key}
                          textValue={`${to}, ${edges}`}
                          aria-label={[`${to}, ${edges}`, trouble].filter(Boolean).join('. ')}
                          className="seam-row"
                          data-seam={row.seam.id}
                          data-under={piece.id}
                        >
                          <TreeItemContent>
                            <span className="seam-badge" aria-hidden="true">
                              {row.number}
                            </span>
                            <span className="seam-text">
                              <span className="seam-to">{to}</span>
                              <span className="muted small">{edges}</span>
                            </span>
                            {trouble && (
                              <span className="seam-trouble" title={trouble} aria-hidden="true">
                                <TriangleAlert size={18} />
                              </span>
                            )}
                          </TreeItemContent>
                        </TreeItem>
                      )
                    })}
                </TreeItem>
              )
            })}
          </Tree>
        </div>
      )}
      <FindSeams session={session} />
    </section>
  )
}
