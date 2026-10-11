import type { AllowanceMode, ImportSizes, Piece, ScaleOption } from '@drapewright/engine'
import { useId, useState } from 'react'
import {
  Button,
  Dialog,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
  ToggleButton,
  ToggleButtonGroup,
} from 'react-aria-components'
import { outlineSize, type PendingImport, type Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'

/** The size to offer first: the file's sample size if it holds it, else the middle one. */
function firstSize(sizes: ImportSizes): string | undefined {
  if (sizes.sample && sizes.names.includes(sizes.sample)) return sizes.sample
  return sizes.names[Math.floor(sizes.names.length / 2)]
}

/**
 * Shown after a file is read: what was found, what was left out and, when
 * the file does not say, which size to take, how big it is and whether its
 * outlines include seam allowance.
 */
export function ImportDialog({ session }: { session: Session }) {
  const { pendingImport } = useStore(session.ui)
  // Unmounted between imports, so every file starts with fresh answers.
  return pendingImport && <ImportForm session={session} pending={pendingImport} />
}

function ImportForm({ session, pending }: { session: Session; pending: PendingImport }) {
  const units = session.engine.document().settings.displayUnits
  const { result, fileName } = pending
  const { scale, allowance, sizes } = result
  const sizesId = useId()

  // A graded file holds every size; only the chosen ones are shown and added.
  const graded = sizes.names.length > 1
  const [chosenSizes, setChosenSizes] = useState<string[]>(() => {
    const first = firstSize(sizes)
    return graded && first ? [first] : []
  })
  const sizeOf = new Map(sizes.pieces.map((ps) => [ps.piece, ps.size]))
  const kept = (id: string) => {
    const size = sizeOf.get(id)
    return !graded || size === undefined || chosenSizes.includes(size)
  }
  const pieces = result.pieces.filter((p) => kept(p.id))
  const notes = result.notes.filter((n) => !('piece' in n) || kept(n.piece))
  // As the pieces will be named: comparing sizes, each says its own.
  const shownName = (p: Piece) => {
    const size = sizeOf.get(p.id)
    return chosenSizes.length > 1 && size !== undefined ? `${p.name} (${size})` : p.name
  }

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
      await session.confirmImport(
        chosen,
        mode && asked.length > 0 ? { mode, width } : null,
        graded ? chosenSizes : null,
      )
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
            {result.pieces.length === 0
              ? t.importing.nothingFound
              : graded
                ? t.importing.foundSized(
                    new Set(result.pieces.map((p) => p.name)).size,
                    sizes.names.length,
                  )
                : t.importing.found(pieces.length)}
          </Heading>
          <p className="muted small">{fileName}</p>
          {graded && (
            <div className="size-choice">
              <p className="group-label" id={sizesId}>
                {t.importing.sizeQuestion}
              </p>
              <p className="muted small">{t.importing.sizeHint}</p>
              <ToggleButtonGroup
                className="size-chips"
                aria-labelledby={sizesId}
                selectionMode="multiple"
                disallowEmptySelection
                selectedKeys={chosenSizes}
                onSelectionChange={(keys) => setChosenSizes(sizes.names.filter((n) => keys.has(n)))}
              >
                {sizes.names.map((n) => (
                  <ToggleButton
                    key={n}
                    id={n}
                    className="size-chip"
                    aria-label={n === sizes.sample ? t.importing.sampleSizeLabel(n) : undefined}
                  >
                    {n}
                    {n === sizes.sample && (
                      <span className="size-sample" aria-hidden="true">
                        {t.importing.sampleSize}
                      </span>
                    )}
                  </ToggleButton>
                ))}
              </ToggleButtonGroup>
            </div>
          )}
          {pieces.length > 0 && (
            <ul className="found-list">
              {pieces.map((p) => (
                <li key={p.id}>{shownName(p)}</li>
              ))}
            </ul>
          )}

          {pieces.length > 0 &&
            (scale.certain ? (
              <p className="muted">{t.importing.sizeCertain}</p>
            ) : (
              <RadioGroup className="scale-choice" value={chosen.basis} onChange={setChoice}>
                <Label className="group-label">{t.importing.scaleQuestion}</Label>
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
                  {t.importing.allowanceQuestion(asked.map(shownName), pieces.length)}
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
