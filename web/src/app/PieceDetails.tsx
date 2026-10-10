// Inspector sections for a piece's notches, grainline and cutting
// instructions.

import type { CutInstruction, Notch, NotchFrom, NotchKind, Piece, PieceView } from '@soom/engine'
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
import { LengthField } from './LengthField.tsx'

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
    <div className="subsection">
      <h3>{t.grain.heading}</h3>
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
    </div>
  )
}

const MATERIALS = ['main', 'lining', 'interfacing', 'contrast'] as const

/** "Main fabric" for known materials, else the text itself. */
export function materialName(material: string): string {
  return t.cut.materials[material] ?? material
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
        <NumberField
          className="field compact"
          value={cut.quantity}
          minValue={1}
          maxValue={99}
          step={1}
          onChange={(quantity) => Number.isFinite(quantity) && onChange({ ...cut, quantity })}
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
        <Select
          className="field"
          selectedKey={
            MATERIALS.includes(cut.material as (typeof MATERIALS)[number]) ? cut.material : null
          }
          onSelectionChange={(material) => onChange({ ...cut, material: String(material) })}
        >
          <Label>{t.cut.material}</Label>
          <Button className="select-button">
            <SelectValue>{() => materialName(cut.material)}</SelectValue>
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

/** "Cut 2 · Main fabric" for the first instruction. */
export function cutSummary(piece: Piece): string {
  const first = piece.cut[0]
  if (!first) return ''
  const hasFold = piece.outline.edges.some((e) => e.kind === 'fold')
  return t.cut.summary(first.quantity, materialName(first.material), first.onFold ?? hasFold)
}
