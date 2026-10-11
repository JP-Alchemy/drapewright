import type { AllowanceMode, ScaleOption } from '@drapewright/engine'
import { useState } from 'react'
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
import { outlineSize, type PendingImport, type Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'

/**
 * Shown after a file is read: what was found, what was left out and, when
 * the file does not say, how big it is and whether its outlines include
 * seam allowance.
 */
export function ImportDialog({ session }: { session: Session }) {
  const { pendingImport } = useStore(session.ui)
  // Unmounted between imports, so every file starts with fresh answers.
  return pendingImport && <ImportForm session={session} pending={pendingImport} />
}

function ImportForm({ session, pending }: { session: Session; pending: PendingImport }) {
  const units = session.engine.document().settings.displayUnits
  const { result, fileName } = pending
  const { scale, pieces, notes, allowance } = result
  const [choice, setChoice] = useState<string | null>(null)
  const [mode, setMode] = useState<AllowanceMode | null>(allowance.suggested)
  const asked = pieces.filter((p) => allowance.pieces.includes(p.id))
  const [width, setWidth] = useState(asked[0]?.allowance.seam ?? 0)
  const [adding, setAdding] = useState(false)

  const options = [scale.chosen, ...scale.alternatives]
  const chosen = options.find((o) => o.basis === choice) ?? scale.chosen
  const largest = [...pieces].sort((a, b) => {
    const sa = outlineSize(a)
    const sb = outlineSize(b)
    return sb.width * sb.height - sa.width * sa.height
  })[0]
  const pieceName = (id: string) => pieces.find((p) => p.id === id)?.name ?? id
  const answered = asked.length === 0 || mode !== null

  const sizeAt = (option: ScaleOption) => {
    if (!largest) return ''
    const factor = option.mmPerUnit / scale.chosen.mmPerUnit
    const { width, height } = outlineSize(largest)
    return t.importing.wouldBe(
      largest.name,
      `${formatLength(width * factor, units)} × ${formatLength(height * factor, units)}`,
    )
  }

  const close = () => session.cancelImport()
  const add = async () => {
    setAdding(true)
    try {
      await session.confirmImport(chosen, mode && asked.length > 0 ? { mode, width } : null)
    } finally {
      setAdding(false)
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

          {asked.length > 0 && (
            <>
              <RadioGroup
                className="scale-choice"
                value={mode}
                onChange={(v) => setMode(v as AllowanceMode)}
              >
                <Label className="group-label">
                  {t.importing.allowanceQuestion(
                    asked.map((p) => p.name),
                    pieces.length,
                  )}
                </Label>
                <p className="muted small">
                  {allowance.suggested === 'included'
                    ? t.importing.allowanceSuggested(asked.length)
                    : t.importing.allowanceWhy(asked.length)}
                </p>
                <Radio value="add" className="radio-card">
                  <span className="radio-title">{t.importing.allowanceAdd}</span>
                  <span className="muted small">{t.importing.allowanceAddDetail}</span>
                </Radio>
                <Radio value="included" className="radio-card">
                  <span className="radio-title">{t.importing.allowanceIncluded}</span>
                  <span className="muted small">{t.importing.allowanceIncludedDetail}</span>
                </Radio>
              </RadioGroup>
              <LengthField
                label={t.importing.allowanceWidth}
                value={width}
                units={units}
                onChange={setWidth}
              />
            </>
          )}

          {notes.length > 0 && (
            <ul className="notes">
              {notes.map((n, i) => (
                <li key={i}>{t.importing.note(n, pieceName, units)}</li>
              ))}
            </ul>
          )}

          {!answered && <p className="muted small">{t.importing.allowanceNeeded}</p>}
          <div className="dialog-actions">
            <Button className="button" onPress={close}>
              {t.importing.cancel}
            </Button>
            {pieces.length > 0 && (
              <Button
                className="button primary"
                onPress={add}
                isDisabled={adding || !answered}
                autoFocus
              >
                {t.importing.add(pieces.length)}
              </Button>
            )}
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}
