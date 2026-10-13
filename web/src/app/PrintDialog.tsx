import type {
  Assembly,
  NotchStyle,
  Orientation,
  PaperName,
  PrintPlan,
  PrintSettings,
} from '@drapewright/engine'
import { ChevronDown, Printer } from 'lucide-react'
import { useEffect, useState } from 'react'
import {
  Button,
  Dialog,
  DialogTrigger,
  Heading,
  Label,
  ListBox,
  ListBoxItem,
  Modal,
  ModalOverlay,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Switch,
} from 'react-aria-components'
import { fileName, saveFile } from '../platform/save-file.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'
import { paperLabel, pdfTexts } from './print.ts'

const PAPERS: PaperName[] = ['a4', 'letter', 'a3', 'tabloid', 'a0']

/** The Print button, and behind it the print settings, a preview and Save PDF. */
export function PrintDialog({ session }: { session: Session }) {
  useEngineState(session)
  const empty = session.engine.document().pieces.length === 0
  return (
    <DialogTrigger>
      <Button className="button" isDisabled={empty}>
        <Printer size={18} aria-hidden="true" />
        <span className="button-label minor">{t.actions.print}</span>
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal wide">
          <Dialog className="dialog">
            {({ close }) => <PrintForm session={session} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function PrintForm({ session, close }: { session: Session; close: () => void }) {
  useEngineState(session)
  const doc = session.engine.document()
  const print = doc.print
  const units = doc.settings.displayUnits
  const [plan, setPlan] = useState<{ for: typeof doc; plan: PrintPlan } | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  // Lays the pages out again after every change, in the IO worker.
  useEffect(() => {
    let current = true
    session.io.call('planPrint', doc).then(
      (p) => {
        if (!current) return
        setPlan({ for: doc, plan: p })
        setProblem(null)
      },
      (error: unknown) => {
        if (!current) return
        setPlan(null)
        setProblem(message(error))
      },
    )
    return () => {
      current = false
    }
  }, [session, doc])

  const set = (change: Partial<PrintSettings>) =>
    session.dispatch(
      { type: 'setPrintSettings', print: { ...print, ...change } },
      { coalesce: 'print-settings' },
    )
  // The last layout stays on show while the next one is worked out; only
  // an up-to-date one can be saved.
  const ready = plan?.plan ?? null
  const upToDate = plan?.for === doc
  const printed = ready ? ready.pages.filter((p) => p.printed).length : 0
  const skipped = ready ? ready.pages.length - printed : 0

  const save = async () => {
    if (!ready || !upToDate) return
    setSaving(true)
    try {
      const bytes = await session.io.call('exportPdf', doc, pdfTexts(doc, ready))
      const name = fileName(doc.meta.name, 'pdf')
      saveFile(bytes, name, 'application/pdf')
      session.toast(t.print.saved(name))
      close()
    } catch (error) {
      setProblem(t.print.failed(message(error)))
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <Heading slot="title">{t.print.heading}</Heading>
      <div className="print-layout">
        <div className="print-settings">
          <Select
            className="field"
            selectedKey={typeof print.paper === 'string' ? print.paper : null}
            onSelectionChange={(paper) => set({ paper: paper as PaperName })}
          >
            <Label>{t.print.paper}</Label>
            <Button className="select-button">
              <SelectValue>{() => paperLabel(print.paper)}</SelectValue>
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            <Popover className="popover">
              <ListBox className="menu">
                {PAPERS.map((p) => (
                  <ListBoxItem key={p} id={p} className="menu-item">
                    {t.print.papers[p]}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>

          <RadioGroup
            className="segmented"
            orientation="horizontal"
            value={print.orientation}
            onChange={(v) => set({ orientation: v as Orientation })}
          >
            <Label className="group-label">{t.print.orientation}</Label>
            <div className="segments">
              <Radio value="portrait" className="segment">
                {t.print.portrait}
              </Radio>
              <Radio value="landscape" className="segment">
                {t.print.landscape}
              </Radio>
            </div>
          </RadioGroup>

          <RadioGroup
            className="scale-choice"
            value={print.assembly}
            onChange={(v) => set({ assembly: v as Assembly })}
          >
            <Label className="group-label">{t.print.joining}</Label>
            <Radio value="trim" className="radio-card">
              <span className="radio-title">{t.print.trim}</span>
              <span className="muted small">{t.print.trimDetail}</span>
            </Radio>
            <Radio value="overlap" className="radio-card">
              <span className="radio-title">{t.print.overlap}</span>
              <span className="muted small">{t.print.overlapDetail}</span>
            </Radio>
          </RadioGroup>
          {print.assembly === 'overlap' && (
            <LengthField
              label={t.print.overlapWidth}
              value={print.overlap}
              units={units}
              min={5}
              onChange={(overlap) => set({ overlap })}
            />
          )}
          <LengthField
            label={t.print.margin}
            value={print.margin}
            units={units}
            description={t.print.marginHint}
            onChange={(margin) => set({ margin })}
          />

          <RadioGroup
            className="segmented"
            orientation="horizontal"
            value={print.notchStyle}
            onChange={(v) => set({ notchStyle: v as NotchStyle })}
          >
            <Label className="group-label">{t.print.notches}</Label>
            <div className="segments">
              <Radio value="slit" className="segment">
                {t.print.slit}
              </Radio>
              <Radio value="v" className="segment">
                {t.print.v}
              </Radio>
            </div>
          </RadioGroup>

          {(
            [
              ['stitchLine', t.print.stitchLine],
              ['testSquare', t.print.testSquare],
              ['pageMap', t.print.pageMap],
              ['skipEmptyPages', t.print.skipEmpty],
            ] as const
          ).map(([key, label]) => (
            <Switch
              key={key}
              className="switch"
              isSelected={print[key]}
              onChange={(on) => set({ [key]: on })}
            >
              <span className="switch-track" aria-hidden="true" />
              {label}
            </Switch>
          ))}
        </div>

        <div className="print-summary">
          {ready ? (
            <>
              <PagePreview session={session} plan={ready} />
              <p data-testid="print-summary">
                <strong>
                  {t.print.summary(printed + (ready.cover ? 1 : 0))} of {paperLabel(print.paper)}
                </strong>
                {ready.cover && <span className="muted"> ({t.print.cover})</span>}
              </p>
              <p className="muted small">
                {t.print.grid(ready.columns, ready.rows)}
                {skipped > 0 && ` · ${t.print.skipped(skipped)}`}
              </p>
              {ready.unprintable.length > 0 && (
                <p className="muted small">{t.print.unprintable(ready.unprintable)}</p>
              )}
            </>
          ) : (
            <p className={problem ? 'error' : 'muted'} role={problem ? 'alert' : undefined}>
              {problem ?? t.print.planning}
            </p>
          )}
          {ready && problem && (
            <p className="error" role="alert">
              {problem}
            </p>
          )}
          <p className="muted small">{t.print.hint}</p>
        </div>
      </div>
      <div className="dialog-actions">
        <Button className="button" onPress={close}>
          {t.actions.close}
        </Button>
        <Button className="button primary" onPress={save} isDisabled={!upToDate || saving}>
          {saving ? t.print.saving : t.print.save}
        </Button>
      </div>
    </>
  )
}

/** The sheet as it will be cut into pages: page outlines, labels and pieces. */
function PagePreview({ session, plan }: { session: Session; plan: PrintPlan }) {
  const width = (plan.columns - 1) * plan.stepX + plan.printableWidth
  const height = (plan.rows - 1) * plan.stepY + plan.printableHeight
  const label = Math.min(plan.printableWidth, plan.printableHeight) / 4
  return (
    <svg
      className="print-preview"
      viewBox={`-5 -5 ${width + 10} ${height + 10}`}
      role="img"
      aria-label={t.print.preview(plan.pages.filter((p) => p.printed).length)}
    >
      {plan.pages.map((p) => (
        <rect
          key={p.label}
          x={p.column * plan.stepX}
          y={p.row * plan.stepY}
          width={plan.printableWidth}
          height={plan.printableHeight}
          className={p.printed ? 'preview-page' : 'preview-page skipped'}
        />
      ))}
      {plan.placements.map((pl) => {
        const view = session.engine.pieceView(pl.piece)
        if (!view) return null
        const points: string[] = []
        for (let i = 0; i < view.cutting.length; i += 2) {
          points.push(`${view.cutting[i]},${view.cutting[i + 1]}`)
        }
        return (
          <polygon
            key={pl.piece}
            points={points.join(' ')}
            transform={`translate(${pl.x} ${pl.y}) rotate(${pl.rotation})`}
            className="preview-piece"
          />
        )
      })}
      {plan.pages
        .filter((p) => p.printed)
        .map((p) => (
          <text
            key={p.label}
            x={p.column * plan.stepX + plan.printableWidth / 2}
            y={p.row * plan.stepY + plan.printableHeight / 2}
            fontSize={label}
            className="preview-label"
          >
            {p.label}
          </text>
        ))}
    </svg>
  )
}
