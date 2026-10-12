// Inspector sections for a piece's notches, grainline and cutting
// instructions.

import type {
  CutInstruction,
  Notch,
  NotchFrom,
  NotchKind,
  Piece,
  PieceView,
} from '@drapewright/engine'
import {
  Button,
  Group,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  NumberField,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Switch,
} from 'react-aria-components'
import { ChevronDown, Minus, Plus } from 'lucide-react'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { LengthField, MixedNumberField } from './LengthField.tsx'

export function NotchSection({
  session,
  piece,
  view,
  notchId,
}: {
  session: Session
  piece: Piece
  view: PieceView
  notchId: string
}) {
  const units = session.engine.document().settings.displayUnits
  const notch = piece.notches.find((n) => n.id === notchId)
  const length = view.edges.find((e) => e.id === notch?.edge)?.length
  if (!notch || length === undefined) return null
  const set = (next: Notch, coalesce?: string) =>
    session.dispatch(
      { type: 'setNotch', piece: piece.id, notch: next },
      coalesce ? { coalesce } : undefined,
    )

  return (
    <section className="panel-section inspector" aria-labelledby="notch-heading">
      <h2 id="notch-heading">{t.notch.heading}</h2>
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        value={notch.kind}
        onChange={(kind) => set({ ...notch, kind: kind as NotchKind })}
      >
        <Label className="group-label">{t.notch.kind}</Label>
        <div className="segments">
          {(['single', 'double', 'triple'] as const).map((k) => (
            <Radio key={k} value={k} className="segment">
              {t.notch.kinds[k]}
            </Radio>
          ))}
        </div>
      </RadioGroup>
      <LengthField
        label={t.notch.distance}
        value={notch.at}
        units={units}
        onChange={(at) => set({ ...notch, at: Math.min(at, length) }, `notch-at:${notch.id}`)}
      />
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        value={notch.from}
        onChange={(from) => {
          // Keep the notch where it is; only the corner it is measured from changes.
          if (from !== notch.from) set({ ...notch, from: from as NotchFrom, at: length - notch.at })
        }}
      >
        <Label className="group-label">{t.notch.measuredFrom}</Label>
        <div className="segments">
          {(['start', 'end'] as const).map((f) => (
            <Radio key={f} value={f} className="segment">
              {t.notch.from[f]}
            </Radio>
          ))}
        </div>
      </RadioGroup>
      <Button
        className="button danger"
        onPress={() => session.dispatch({ type: 'deleteNotch', piece: piece.id, notch: notch.id })}
      >
        {t.notch.delete}
      </Button>
    </section>
  )
}

/** A vertical grainline through the middle of the piece, 60% of its height. */
function defaultGrainline(view: PieceView) {
  const [x0, y0, x1, y1] = view.outlineBounds
  const cx = (x0 + x1) / 2
  const h = (y1 - y0) * 0.3
  const cy = (y0 + y1) / 2
  return {
    from: [cx, cy - h] as [number, number],
    to: [cx, cy + h] as [number, number],
    oneWay: false,
  }
}

export function GrainSection({
  session,
  piece,
  view,
}: {
  session: Session
  piece: Piece
  view: PieceView
}) {
  const g = piece.grainline
  return (
    <>
      {g ? (
        <>
          <Switch
            className="switch"
            isSelected={g.oneWay}
            onChange={(oneWay) =>
              session.dispatch({
                type: 'setGrainline',
                piece: piece.id,
                grainline: { ...g, oneWay },
              })
            }
          >
            <span className="switch-track" aria-hidden="true" />
            {t.grain.oneWay}
          </Switch>
          <Button
            className="link-button"
            onPress={() =>
              session.dispatch({ type: 'setGrainline', piece: piece.id, grainline: null })
            }
          >
            {t.grain.remove}
          </Button>
        </>
      ) : (
        <>
          <p className="muted small">{t.grain.none}</p>
          <Button
            className="button"
            onPress={() =>
              session.dispatch({
                type: 'setGrainline',
                piece: piece.id,
                grainline: defaultGrainline(view),
              })
            }
          >
            {t.grain.add}
          </Button>
        </>
      )}
    </>
  )
}

/** "One way", "Both ways" or "None". */
export function grainSummary(piece: Piece): string {
  const g = piece.grainline
  return g ? (g.oneWay ? t.grain.oneWayShort : t.grain.bothWays) : t.grain.noneShort
}

const MATERIALS = ['main', 'lining', 'interfacing', 'contrast'] as const

/** "Main fabric" for known materials, else the text itself. */
export function materialName(material: string): string {
  return t.cut.materials[material] ?? material
}

const MOST = 99
const count = (n: number) => Math.min(MOST, Math.max(1, Math.round(n)))

/** How many to cut: `null` where pieces chosen together differ. */
function QuantityField({
  value,
  onChange,
  onStep,
}: {
  value: number | null
  onChange(quantity: number): void
  onStep(by: number): void
}) {
  if (value === null) {
    return (
      <MixedNumberField
        label={t.cut.quantity}
        step={1}
        onStep={onStep}
        onSet={(n) => onChange(count(n))}
        parse={(text) => {
          const n = Number.parseInt(text, 10)
          return Number.isFinite(n) ? n : null
        }}
      />
    )
  }
  return (
    <NumberField
      className="field compact"
      value={value}
      minValue={1}
      maxValue={MOST}
      step={1}
      onChange={(quantity) => Number.isFinite(quantity) && onChange(quantity)}
    >
      <Label>{t.cut.quantity}</Label>
      <Group className="number-group">
        <Button slot="decrement" className="step-button">
          <Minus size={16} aria-hidden="true" />
        </Button>
        <Input className="number-input" />
        <Button slot="increment" className="step-button">
          <Plus size={16} aria-hidden="true" />
        </Button>
      </Group>
    </NumberField>
  )
}

/** Which fabric: `null` where pieces chosen together differ. */
function MaterialSelect({
  value,
  onChange,
}: {
  value: string | null
  onChange(material: string): void
}) {
  return (
    <Select
      className="field compact"
      selectedKey={MATERIALS.includes(value as (typeof MATERIALS)[number]) ? value : null}
      onSelectionChange={(material) => onChange(String(material))}
    >
      <Label>{t.cut.material}</Label>
      <Button className="select-button">
        <SelectValue>
          {() => (value === null ? t.inspector.mixed : materialName(value))}
        </SelectValue>
        <ChevronDown size={16} aria-hidden="true" />
      </Button>
      <Popover className="popover">
        <ListBox className="menu">
          {MATERIALS.map((m) => (
            <ListBoxItem key={m} id={m} className="menu-item">
              {materialName(m)}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </Select>
  )
}

function CutRow({
  cut,
  hasFold,
  onChange,
  onRemove,
}: {
  cut: CutInstruction
  hasFold: boolean
  onChange(next: CutInstruction): void
  onRemove?: () => void
}) {
  const mirrored = cut.mirrored ?? cut.quantity >= 2
  const onFold = cut.onFold ?? hasFold
  return (
    <div className="cut-row">
      <div className="cut-line">
        <QuantityField
          value={cut.quantity}
          onChange={(quantity) => onChange({ ...cut, quantity })}
          onStep={(by) => onChange({ ...cut, quantity: count(cut.quantity + by) })}
        />
        <MaterialSelect
          value={cut.material}
          onChange={(material) => onChange({ ...cut, material })}
        />
      </div>
      {cut.quantity >= 2 && (
        <Switch
          className="switch"
          isSelected={mirrored}
          onChange={(m) => onChange({ ...cut, mirrored: m })}
        >
          <span className="switch-track" aria-hidden="true" />
          {t.cut.mirrored}
        </Switch>
      )}
      {hasFold && (
        <Switch
          className="switch"
          isSelected={onFold}
          onChange={(f) => onChange({ ...cut, onFold: f })}
        >
          <span className="switch-track" aria-hidden="true" />
          {t.cut.onFold}
        </Switch>
      )}
      {onRemove && (
        <Button className="link-button" onPress={onRemove}>
          {t.cut.remove}
        </Button>
      )}
    </div>
  )
}

export function CutSection({ session, piece }: { session: Session; piece: Piece }) {
  const hasFold = piece.outline.edges.some((e) => e.kind === 'fold')
  const setCut = (cut: CutInstruction[]) =>
    session.dispatch({ type: 'setCut', piece: piece.id, cut })
  return (
    <div className="subsection">
      <h3>{t.cut.heading}</h3>
      {piece.cut.map((cut, i) => (
        <CutRow
          key={i}
          cut={cut}
          hasFold={hasFold}
          onChange={(next) => setCut(piece.cut.map((c, k) => (k === i ? next : c)))}
          onRemove={
            piece.cut.length > 1 ? () => setCut(piece.cut.filter((_, k) => k !== i)) : undefined
          }
        />
      ))}
      <Button
        className="link-button"
        onPress={() =>
          setCut([
            ...piece.cut,
            {
              material: piece.cut.some((c) => c.material === 'lining') ? 'interfacing' : 'lining',
              quantity: piece.cut[0]?.quantity ?? 1,
              mirrored: null,
              onFold: null,
            },
          ])
        }
      >
        {t.cut.add}
      </Button>
    </div>
  )
}

/** What a piece that says nothing about cutting is cut as. */
const MAIN_ONCE: CutInstruction = { material: 'main', quantity: 1, mirrored: null, onFold: null }

/** One value if all of `xs` are it, else `null` (mixed). */
const shared = <T,>(xs: T[]): T | null => (xs.every((x) => x === xs[0]) ? (xs[0] ?? null) : null)

/**
 * How several pieces chosen together are cut: each one's first line (its
 * main fabric). A value they don't share shows as mixed; a change applies
 * to all of them, as one undo step for a run of changes.
 */
export function ManyCutSection({ session, pieces }: { session: Session; pieces: Piece[] }) {
  const first = (p: Piece) => p.cut[0] ?? MAIN_ONCE
  const step = { coalesce: `cut:${pieces.map((p) => p.id).join(',')}` }
  const change = (edit: (c: CutInstruction) => CutInstruction) => {
    for (const p of pieces) {
      const cut = p.cut.length > 0 ? p.cut : [MAIN_ONCE]
      session.dispatch(
        { type: 'setCut', piece: p.id, cut: cut.map((c, i) => (i === 0 ? edit(c) : c)) },
        step,
      )
    }
  }
  const quantities = pieces.map((p) => first(p).quantity)
  const mirrored = pieces.map((p) => first(p).mirrored ?? first(p).quantity >= 2)
  return (
    <div className="subsection">
      <h3>{t.cut.heading}</h3>
      <div className="cut-row">
        <div className="cut-line">
          <QuantityField
            value={shared(quantities)}
            onChange={(quantity) => change((c) => ({ ...c, quantity }))}
            onStep={(by) => change((c) => ({ ...c, quantity: count(c.quantity + by) }))}
          />
          <MaterialSelect
            value={shared(pieces.map((p) => first(p).material))}
            onChange={(material) => change((c) => ({ ...c, material }))}
          />
        </div>
        {quantities.some((q) => q >= 2) && (
          <>
            <Switch
              className="switch"
              isSelected={mirrored.every(Boolean)}
              onChange={(m) => change((c) => ({ ...c, mirrored: m }))}
            >
              <span className="switch-track" aria-hidden="true" />
              {t.cut.mirrored}
            </Switch>
            {shared(mirrored) === null && <p className="muted small">{t.cut.someMirrored}</p>}
          </>
        )}
      </div>
    </div>
  )
}

/** "Cut 2", or "Cut 1 on the fold", for the first instruction. */
export function cutShort(piece: Piece): string {
  const first = piece.cut[0]
  const hasFold = piece.outline.edges.some((e) => e.kind === 'fold')
  return t.cut.short(first?.quantity ?? 1, first?.onFold ?? hasFold)
}

/** "Cut 2 · Main fabric" for the first instruction. */
export function cutSummary(piece: Piece): string {
  const first = piece.cut[0]
  if (!first) return ''
  const hasFold = piece.outline.edges.some((e) => e.kind === 'fold')
  return t.cut.summary(first.quantity, materialName(first.material), first.onFold ?? hasFold)
}
