// A pattern's sizes on the table side by side ("Front (8)", "Front (10)"):
// blended between, as on import, into a new set of pieces cut in a
// different size at each level. The sizes stay as they are.

import type { Piece } from '@drapewright/engine'
import { Blend } from 'lucide-react'
import { Button, Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { SizeFinder } from './SizeFinder.tsx'
import { blendedPieces, sizesOnTable } from './sizes.ts'

const text = t.sizing

/** Offered where `chosen` are among a pattern's sizes on the table. */
export function TableSizes({ session, chosen }: { session: Session; chosen: Piece[] }) {
  const found = sizesOnTable(session.engine.document().pieces)
  if (!found || !chosen.some((p) => found.sizeOf.has(p.id))) return null
  const names = found.sizes.map((s) => s.size)
  return (
    <div className="table-sizes">
      <p className="muted small">{text.tableHint(names)}</p>
      <DialogTrigger>
        <Button className="button">
          <Blend size={18} aria-hidden="true" />
          {text.tableOpen}
        </Button>
        <ModalOverlay isDismissable className="modal-overlay">
          <Modal className="modal">
            <Dialog className="dialog">
              {({ close }) => (
                <>
                  <Heading slot="title">{text.tableHeading}</Heading>
                  <SizeFinder
                    session={session}
                    // As they are now: altered or moved since, say.
                    sizePieces={async () =>
                      sizesOnTable(session.engine.document().pieces)?.sizes ?? []
                    }
                    ready
                    onBlend={(blended, at) => {
                      const now = sizesOnTable(session.engine.document().pieces)
                      const { pieces, said } = blendedPieces(blended, at, (id) =>
                        now?.sizeOf.get(id),
                      )
                      if (!session.addBeside(pieces)) return
                      session.notify({ kind: 'info', text: said })
                      close()
                    }}
                  />
                  <div className="dialog-actions">
                    <Button className="button" onPress={close}>
                      {t.actions.close}
                    </Button>
                  </div>
                </>
              )}
            </Dialog>
          </Modal>
        </ModalOverlay>
      </DialogTrigger>
    </div>
  )
}
