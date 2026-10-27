import type { EdgeKind } from '@drapewright/engine'
import { useRef } from 'react'
import { Menu, MenuItem, Popover, Separator } from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'

type Action =
  | 'turn'
  | 'turnBack'
  | 'straighten'
  | 'delete'
  | 'open'
  | 'fit'
  | 'selectAll'
  | 'addCorner'
  | 'removeCorner'
  | 'seam'
  | 'hem'
  | 'fold'
  | 'addNotch'
  | 'deleteNotch'

const KIND_LABEL: Record<EdgeKind, string> = {
  seam: t.menu.makeSeam,
  hem: t.menu.makeHem,
  fold: t.menu.makeFold,
}

/**
 * Long-press or right-click menu for whatever is under the pointer; on one
 * of several chosen pieces, for all of them.
 */
export function ContextMenu({ session, onOpen }: { session: Session; onOpen: () => void }) {
  const { menu } = useStore(session.ui)
  const anchor = useRef<HTMLDivElement>(null)
  const hit = menu?.hit ?? null
  const many = menu?.pieces ?? null
  const anyPieces = session.engine.document().pieces.length > 0
  const piece = hit ? session.engine.document().pieces.find((p) => p.id === hit.piece) : undefined
  const edge =
    hit && (hit.kind === 'edge' || hit.kind === 'corner')
      ? piece?.outline.edges.find((e) => e.id === hit.edge)
      : undefined

  const run = (action: Action) => {
    session.openMenu(null)
    if (!hit) {
      if (action === 'open') onOpen()
      if (action === 'fit') session.fitAll()
      if (action === 'selectAll') session.select(session.engine.document().pieces.map((p) => p.id))
      return
    }
    if (many) {
      session.select(many)
      if (action === 'turn' || action === 'turnBack') {
        session.turnSelection(action === 'turn' ? 90 : -90)
      }
      if (action === 'straighten') session.straightenPieces(many)
      if (action === 'delete') session.deleteSelection()
      return
    }
    switch (action) {
      case 'straighten':
        session.straightenPieces([hit.piece])
        break
      case 'turn':
      case 'turnBack':
        session.dispatch({
          type: 'rotatePiece',
          piece: hit.piece,
          degrees: action === 'turn' ? 90 : -90,
        })
        break
      case 'delete':
        session.dispatch({ type: 'deletePieces', ids: [hit.piece] })
        break
      case 'addCorner':
        if (hit.kind === 'edge') {
          session.dispatch({ type: 'splitEdge', piece: hit.piece, edge: hit.edge, at: hit.along })
        }
        break
      case 'removeCorner':
        if (hit.kind === 'corner') {
          session.dispatch({ type: 'joinEdges', piece: hit.piece, edge: hit.edge })
        }
        break
      case 'addNotch':
        if (hit.kind === 'edge') {
          const length =
            session.scenePiece(hit.piece)?.view.edges.find((e) => e.id === hit.edge)?.length ?? 0
          const fromEnd = hit.along > length / 2
          session.dispatch({
            type: 'addNotch',
            piece: hit.piece,
            edge: hit.edge,
            at: fromEnd ? length - hit.along : hit.along,
            from: fromEnd ? 'end' : 'start',
            kind: 'single',
          })
        }
        break
      case 'deleteNotch':
        if (hit.kind === 'notch')
          session.dispatch({ type: 'deleteNotch', piece: hit.piece, notch: hit.notch })
        break
      case 'seam':
      case 'hem':
      case 'fold':
        if (hit.kind === 'edge') {
          session.dispatch({ type: 'setEdgeKind', piece: hit.piece, edge: hit.edge, kind: action })
        }
        break
    }
  }

  const pieceItems = (
    <>
      <MenuItem id="turn" className="menu-item">
        {t.menu.turn}
      </MenuItem>
      <MenuItem id="turnBack" className="menu-item">
        {t.menu.turnBack}
      </MenuItem>
      <MenuItem id="straighten" className="menu-item">
        {t.inspector.straighten}
      </MenuItem>
      <MenuItem id="delete" className="menu-item danger">
        {many ? t.menu.deletePieces(many.length) : t.menu.deletePiece}
      </MenuItem>
    </>
  )

  return (
    <>
      <div
        ref={anchor}
        className="menu-anchor"
        style={{ left: menu?.x ?? 0, top: menu?.y ?? 0 }}
        aria-hidden="true"
      />
      <Popover
        triggerRef={anchor}
        isOpen={!!menu}
        onOpenChange={(open) => !open && session.openMenu(null)}
        placement="bottom start"
        className="popover"
      >
        <Menu
          // Each kind of menu has different items: give each its own
          // collection rather than swapping items in place while closing.
          key={many ? `many:${many.length}` : `${hit?.kind ?? 'table'}:${edge?.kind ?? ''}`}
          className="menu"
          aria-label={t.menu.label}
          onAction={(key) => run(key as Action)}
          autoFocus="first"
        >
          {!hit && (
            <>
              <MenuItem id="open" className="menu-item">
                {t.actions.open}
              </MenuItem>
              <MenuItem id="fit" className="menu-item">
                {t.actions.fitAll}
              </MenuItem>
              {anyPieces && (
                <MenuItem id="selectAll" className="menu-item">
                  {t.pieces.selectAll}
                </MenuItem>
              )}
            </>
          )}
          {!many && hit?.kind === 'edge' && edge && (
            <>
              <MenuItem id="addNotch" className="menu-item">
                {t.menu.addNotch}
              </MenuItem>
              <MenuItem id="addCorner" className="menu-item">
                {t.menu.addCorner}
              </MenuItem>
              {(['seam', 'hem', 'fold'] as const)
                .filter((k) => k !== edge.kind)
                .map((k) => (
                  <MenuItem key={k} id={k} className="menu-item">
                    {KIND_LABEL[k]}
                  </MenuItem>
                ))}
              <Separator className="menu-separator" />
            </>
          )}
          {!many && hit?.kind === 'notch' && (
            <>
              <MenuItem id="deleteNotch" className="menu-item danger">
                {t.menu.deleteNotch}
              </MenuItem>
              <Separator className="menu-separator" />
            </>
          )}
          {!many && hit?.kind === 'corner' && (
            <>
              <MenuItem id="removeCorner" className="menu-item">
                {t.menu.removeCorner}
              </MenuItem>
              <Separator className="menu-separator" />
            </>
          )}
          {hit && pieceItems}
        </Menu>
      </Popover>
    </>
  )
}
