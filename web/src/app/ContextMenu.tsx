import { useRef } from 'react'
import { Menu, MenuItem, Popover } from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'

type Action = 'turn' | 'turnBack' | 'delete' | 'open' | 'fit'

/** Long-press or right-click menu for whatever is under the pointer. */
export function ContextMenu({ session, onOpen }: { session: Session; onOpen: () => void }) {
  const { menu } = useStore(session.ui)
  const anchor = useRef<HTMLDivElement>(null)
  const hit = menu?.hit ?? null

  const run = (action: Action) => {
    session.openMenu(null)
    switch (action) {
      case 'turn':
      case 'turnBack':
        if (hit)
          session.dispatch({
            type: 'rotatePiece',
            piece: hit.piece,
            degrees: action === 'turn' ? 90 : -90,
          })
        break
      case 'delete':
        if (hit) session.dispatch({ type: 'deletePieces', ids: [hit.piece] })
        break
      case 'open':
        onOpen()
        break
      case 'fit':
        session.fitAll()
        break
    }
  }

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
          // Piece and table menus have different items: give each its own
          // collection rather than swapping items in place while closing.
          key={hit ? 'piece' : 'table'}
          className="menu"
          aria-label={t.menu.label}
          onAction={(key) => run(key as Action)}
          autoFocus="first"
        >
          {hit ? (
            <>
              <MenuItem id="turn" className="menu-item">
                {t.menu.turn}
              </MenuItem>
              <MenuItem id="turnBack" className="menu-item">
                {t.menu.turnBack}
              </MenuItem>
              <MenuItem id="delete" className="menu-item danger">
                {t.menu.deletePiece}
              </MenuItem>
            </>
          ) : (
            <>
              <MenuItem id="open" className="menu-item">
                {t.actions.open}
              </MenuItem>
              <MenuItem id="fit" className="menu-item">
                {t.actions.fitAll}
              </MenuItem>
            </>
          )}
        </Menu>
      </Popover>
    </>
  )
}
