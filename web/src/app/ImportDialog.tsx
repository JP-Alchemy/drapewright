import type { AllowanceMode, ImportSizes, PageUse, Piece, ScaleOption } from '@drapewright/engine'
import { useId, useState } from 'react'
import { Check } from 'lucide-react'
import {
  Button,
  Checkbox,
  CheckboxGroup,
  Dialog,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
  Switch,
  TextArea,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
} from 'react-aria-components'
import { outlineSize, type PendingImport, type Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { groupNotes } from './import-notes.ts'
import { LengthField } from './LengthField.tsx'
import { SizeFinder } from './SizeFinder.tsx'
import { blendedPieces } from './sizes.ts'
import { importReport, mailto } from './feedback.ts'
import { fileName, saveFile } from '../platform/save-file.ts'
import { remember, remembered } from '../platform/remember.ts'
import { askew } from '../state/straighten.ts'

/** Whether pieces drawn a little askew are straightened as they come in, as last chosen. */
const UPRIGHT = 'drapewright.import.upright'

/**
 * A pattern that didn't open right, reported: what's wrong, in the
 * person's words, and what was read from the file, by email (in their own
 * mail) or saved to attach.
 */
function NotRight({ pending }: { pending: PendingImport }) {
  const text = t.feedback
  const [open, setOpen] = useState(false)
  const [said, setSaid] = useState('')
  if (!open) {
    return (
      <p className="small">
        <Button className="link-button" onPress={() => setOpen(true)}>
          {text.notRight}
        </Button>
      </p>
    )
  }
  const report = importReport(pending)
  return (
    <section className="not-right" aria-label={text.notRight}>
      <p className="muted small">{text.notRightHow}</p>
      <TextField className="field" value={said} onChange={setSaid}>
        <Label>{text.whatsWrong}</Label>
        <TextArea className="text-area" rows={3} placeholder={text.whatsWrongPlaceholder} />
      </TextField>
      <div className="feedback-links">
        <a
          className="button"
          href={mailto(text.reportSubject(pending.fileName), text.reportBody(said, report))}
        >
          {text.emailReport}
        </a>
        <Button
          className="button"
          onPress={() =>
            saveFile(
              new TextEncoder().encode(text.reportBody(said, report)),
              fileName(`${pending.fileName} report`, 'txt'),
              'text/plain',
            )
          }
        >
          {text.saveReport}
        </Button>
      </div>
    </section>
  )
}

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
  // Unmounted between imports, and when the file's pages are read again,
  // so every reading starts with fresh answers.
  return (
    pendingImport && (
      <ImportForm
        key={pendingImport.pages?.join(',') ?? 'pages as read'}
        session={session}
        pending={pendingImport}
      />
    )
  )
}

/**
 * A PDF's pages: how many were read and which were left out, and the
 * choice of pages to read again (a page of pieces left out, or a page of
 * instructions taken for pattern).
 */
function PagesChoice({ session, pending }: { session: Session; pending: PendingImport }) {
  const pages = pending.result.pages
  const read = pages.filter((p) => p.leftOut === null).map((p) => p.number)
  const [choosing, setChoosing] = useState(false)
  const [chosen, setChosen] = useState<number[]>(read)
  if (pages.length < 2) return null
  const leftOut = pages.filter((p): p is PageUse => p.leftOut !== null && p.leftOut !== 'notChosen')
  const same = chosen.length === read.length && chosen.every((n, i) => n === read[i])
  return (
    <div className="pages-choice">
      <p className="muted small">
        {t.importing.pagesRead(read.length, pages.length)}
        {leftOut.length > 0 && ` ${t.importing.pagesLeftOut(leftOut)}`}{' '}
        {!choosing && (
          <Button className="link-button" onPress={() => setChoosing(true)}>
            {t.importing.choosePages}
          </Button>
        )}
      </p>
      {choosing && (
        <CheckboxGroup
          aria-label={t.importing.pagesToRead}
          value={chosen.map(String)}
          onChange={(v) => setChosen(v.map(Number).sort((a, b) => a - b))}
        >
          <p className="muted small">{t.importing.pagesHint}</p>
          <ul className="found-list choosable pages-list">
            {pages.map((p) => (
              <li key={p.number}>
                <Checkbox value={String(p.number)} className="piece-check">
                  <span className="piece-box" aria-hidden="true">
                    <Check size={14} strokeWidth={3} />
                  </span>
                  <span className="piece-name">{t.importing.page(p)}</span>
                  {p.leftOut && (
                    <span className="piece-size">{t.importing.pageLeftOut(p.leftOut)}</span>
                  )}
                </Checkbox>
              </li>
            ))}
          </ul>
          <div className="pages-actions">
            {pending.pages && (
              <Button className="button" onPress={() => session.readPages(null)}>
                {t.importing.readAllPages}
              </Button>
            )}
            <Button
              className="button"
              isDisabled={chosen.length === 0 || same}
              onPress={() => session.readPages(chosen)}
            >
              {t.importing.readPages(chosen.length)}
            </Button>
          </div>
        </CheckboxGroup>
      )}
    </div>
  )
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
  // Every piece found is a suggestion: she can leave out what isn't one (a
  // diagram, a title box).
  const [leftOut, setLeftOut] = useState<ReadonlySet<string>>(new Set())
  const toAdd = pieces.filter((p) => !leftOut.has(p.id))
  // The pages left out are told of with the pages, when there are several.
  const notes = result.notes.filter(
    (n) =>
      (!('piece' in n) || kept(n.piece)) && !(n.kind === 'pagesLeftOut' && result.pages.length > 1),
  )
  // As the pieces will be named: comparing sizes, each says its own.
  const shownName = (p: Piece) => {
    const size = sizeOf.get(p.id)
    return chosenSizes.length > 1 && size !== undefined ? `${p.name} (${size})` : p.name
  }

  const [choice, setChoice] = useState<string | null>(null)
  const [mode, setMode] = useState<AllowanceMode | null>(allowance.suggested)
  const asked = toAdd.filter((p) => allowance.pieces.includes(p.id))
  const [width, setWidth] = useState(asked[0]?.allowance.seam ?? 0)
  const [adding, setAdding] = useState(false)
  // Pieces drawn a few degrees off on the sheet: their grainlines not
  // quite up and down.
  const tilted = toAdd.filter(askew)
  const [upright, setUpright] = useState(() => remembered<boolean>(UPRIGHT, true))
  const straighten = upright && tilted.length > 0

  const options = [scale.chosen, ...scale.alternatives]
  const chosen = options.find((o) => o.basis === choice) ?? scale.chosen
  const largest = [...pieces].sort((a, b) => {
    const sa = outlineSize(a)
    const sb = outlineSize(b)
    return sb.width * sb.height - sa.width * sa.height
  })[0]
  const pieceName = (id: string) => {
    const piece = pieces.find((p) => p.id === id)
    return piece ? shownName(piece) : id
  }
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
        leftOut.size > 0 ? toAdd.map((p) => p.id) : null,
        straighten,
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
          <PagesChoice session={session} pending={pending} />
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
                {sizes.names.map((n, i) => (
                  <ToggleButton
                    key={n}
                    id={n}
                    className="size-chip"
                    aria-label={n === sizes.sample ? t.importing.sampleSizeLabel(n) : undefined}
                  >
                    {sizes.swatches[i] && (
                      // The size's line, as printed: sizes told apart by colour.
                      <span
                        className="size-swatch"
                        aria-hidden="true"
                        style={{
                          borderTopColor: sizes.swatches[i].colour,
                          borderTopStyle: sizes.swatches[i].dashed ? 'dashed' : 'solid',
                        }}
                      />
                    )}
                    {n}
                    {n === sizes.sample && (
                      <span className="size-sample" aria-hidden="true">
                        {t.importing.sampleSize}
                      </span>
                    )}
                  </ToggleButton>
                ))}
              </ToggleButtonGroup>
              <SizeFinder
                session={session}
                sizePieces={async () => {
                  // Every size's pieces, as they would be added (scale and
                  // allowance answered, named as in the file).
                  const finished = await session.io.call(
                    'finishImport',
                    result,
                    sizes.names,
                    chosen,
                    mode && asked.length > 0 ? { mode, width } : null,
                  )
                  // A piece left out in the size shown is left out of every size.
                  const named = new Map(result.pieces.map((p) => [p.id, p.name]))
                  const unwanted = new Set([...leftOut].map((id) => named.get(id)))
                  return sizes.names.map((size) => ({
                    size,
                    pieces: finished.pieces
                      .map((p) => ({ ...p, name: named.get(p.id) ?? p.name }))
                      .filter((p) => {
                        const own = sizeOf.get(p.id)
                        return !unwanted.has(p.name) && (own === undefined || own === size)
                      }),
                  }))
                }}
                ready={answered}
                onChoose={(size) => setChosenSizes([size])}
                onBlend={(blended, at) => {
                  const { pieces, said } = blendedPieces(blended, at, (id) => sizeOf.get(id))
                  if (session.addImported(pieces, straighten))
                    session.notify({ kind: 'info', text: said })
                }}
              />
            </div>
          )}
          {pieces.length > 0 && (
            <CheckboxGroup
              aria-label={t.importing.piecesToAdd}
              value={toAdd.map((p) => p.id)}
              onChange={(ids) =>
                setLeftOut(new Set(pieces.map((p) => p.id).filter((id) => !ids.includes(id))))
              }
            >
              {pieces.length > 1 && <p className="muted small">{t.importing.untickHint}</p>}
              <ul className="found-list choosable">
                {pieces.map((p) => {
                  const { width: w, height: h } = outlineSize(p)
                  return (
                    <li key={p.id}>
                      <Checkbox value={p.id} className="piece-check">
                        <span className="piece-box" aria-hidden="true">
                          <Check size={14} strokeWidth={3} />
                        </span>
                        <span className="piece-name">{shownName(p)}</span>
                        <span className="piece-size" aria-hidden="true">
                          {formatLength(w, units)} × {formatLength(h, units)}
                        </span>
                      </Checkbox>
                    </li>
                  )
                })}
              </ul>
            </CheckboxGroup>
          )}

          {pieces.length > 0 &&
            (scale.certain ? (
              <p className="muted">
                {scale.reason === 'referenceSheet'
                  ? t.importing.reason(scale.reason)
                  : t.importing.sizeCertain}
              </p>
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

          {tilted.length > 0 && (
            <>
              <Switch
                className="switch"
                isSelected={upright}
                onChange={(on) => {
                  setUpright(on)
                  remember(UPRIGHT, on)
                }}
              >
                <span className="switch-track" aria-hidden="true" />
                {t.importing.straighten}
              </Switch>
              <p className="muted small">
                {t.importing.straightenHint(tilted.map(shownName), toAdd.length)}
              </p>
            </>
          )}

          {notes.length > 0 && (
            <ul className="notes">
              {groupNotes(notes).map(({ note, pieces: about }, i) => (
                <li key={i}>
                  {t.importing.note(note, about.map(pieceName), pieces.length, units)}
                </li>
              ))}
            </ul>
          )}

          <NotRight pending={pending} />
          {!answered && <p className="muted small">{t.importing.allowanceNeeded}</p>}
          <div className="dialog-actions">
            <Button className="button" onPress={close}>
              {t.importing.cancel}
            </Button>
            {pieces.length > 0 && (
              <Button
                className="button primary"
                onPress={add}
                isDisabled={adding || !answered || toAdd.length === 0}
                autoFocus
              >
                {t.importing.add(toAdd.length)}
              </Button>
            )}
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}
