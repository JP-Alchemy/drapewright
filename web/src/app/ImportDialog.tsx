import type { ScaleOption } from '@soom/engine'
import { useMemo, useState } from 'react'
import {
  Button,
  Dialog,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
} from 'react-aria-components'
import { outlineSize, type Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'

/**
 * Shown after a file is read: what was found, what was left out, and, when
 * the file does not state its size, which reading of the size is right.
 */
export function ImportDialog({ session }: { session: Session }) {
  const { pendingImport } = useStore(session.ui)
  const units = session.engine.document().settings.displayUnits
  const [choice, setChoice] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)

  const options = useMemo(() => {
    if (!pendingImport) return []
    const { scale } = pendingImport.result
    return [scale.chosen, ...scale.alternatives]
  }, [pendingImport])

  if (!pendingImport) return null
  const { result, fileName } = pendingImport
  const { scale, pieces, notes } = result
  const chosen = options.find((o) => o.basis === choice) ?? scale.chosen
  const largest = [...pieces].sort((a, b) => {
    const sa = outlineSize(a)
    const sb = outlineSize(b)
    return sb.width * sb.height - sa.width * sa.height
  })[0]
  const pieceName = (id: string) => pieces.find((p) => p.id === id)?.name ?? id

  const sizeAt = (option: ScaleOption) => {
    if (!largest) return ''
    const factor = option.mmPerUnit / scale.chosen.mmPerUnit
    const { width, height } = outlineSize(largest)
    return t.importing.wouldBe(
      largest.name,
      `${formatLength(width * factor, units)} × ${formatLength(height * factor, units)}`,
    )
  }

  const close = () => {
    setChoice(null)
    session.cancelImport()
  }
  const add = async () => {
    setAdding(true)
    try {
      await session.confirmImport(chosen)
    } finally {
      setAdding(false)
      setChoice(null)
    }
  }

  return (
    <ModalOverlay
      isOpen
      isDismissable
      onOpenChange={(open) => !open && close()}
      className="modal-overlay"
    >
      <Modal className="modal">
        <Dialog className="dialog">
          <Heading slot="title">
            {pieces.length > 0 ? t.importing.found(pieces.length) : t.importing.nothingFound}
          </Heading>
          <p className="muted small">{fileName}</p>
          {pieces.length > 0 && (
            <ul className="found-list">
              {pieces.map((p) => (
                <li key={p.id}>{p.name}</li>
              ))}
            </ul>
          )}

          {pieces.length > 0 &&
            (scale.certain ? (
              <p className="muted">{t.importing.sizeCertain}</p>
            ) : (
              <RadioGroup className="scale-choice" value={chosen.basis} onChange={setChoice}>
                <Label className="group-label">{t.importing.sizeQuestion}</Label>
                <p className="muted small">{t.importing.reason(scale.reason)}</p>
                {options.map((o) => (
                  <Radio key={o.basis} value={o.basis} className="radio-card">
                    <span className="radio-title">{t.importing.basis(o.basis)}</span>
                    <span className="muted small">{sizeAt(o)}</span>
                  </Radio>
                ))}
              </RadioGroup>
            ))}

          {notes.length > 0 && (
            <ul className="notes">
              {notes.map((n, i) => (
                <li key={i}>{t.importing.note(n, pieceName, units)}</li>
              ))}
            </ul>
          )}

          <div className="dialog-actions">
            <Button className="button" onPress={close}>
              {t.importing.cancel}
            </Button>
            {pieces.length > 0 && (
              <Button className="button primary" onPress={add} isDisabled={adding} autoFocus>
                {t.importing.add(pieces.length)}
              </Button>
            )}
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}
