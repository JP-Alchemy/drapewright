// The cutting layout: the pieces cut from a fabric laid out on it, as
// they would be cut, and how much of it they take, to know how much to
// buy. Fabric folded in half lengthwise (as most is cut at home), or open;
// a nap or a one-way print keeps every piece the same way up.

import type { DisplayUnits, Layout, Placed } from '@drapewright/engine'
import { ChevronDown, Scissors } from 'lucide-react'
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
  Select,
  SelectValue,
  Switch,
} from 'react-aria-components'
import { remember, remembered } from '../platform/remember.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'
import { materialName } from './PieceDetails.tsx'

const text = t.layout
const KEPT = 'drapewright.layout'

/** Fabric widths sold, mm: metric, or in inches. */
const WIDTHS: Record<'metric' | 'imperial', number[]> = {
  metric: [1120, 1400, 1500],
  imperial: [45 * 25.4, 54 * 25.4, 60 * 25.4],
}

export interface Choice {
  width: number
  folded: boolean
  oneWay: boolean
}

/** The fabric last chosen for the layout, or 150 cm (60″) folded. */
export function keptChoice(units: DisplayUnits): Choice {
  const kept = remembered<Partial<Choice>>(KEPT, {})
  return {
    width:
      typeof kept.width === 'number' && kept.width > 0
        ? kept.width
        : WIDTHS[units === 'in' ? 'imperial' : 'metric'][2]!,
    folded: kept.folded ?? true,
    oneWay: kept.oneWay ?? false,
  }
}

/** The fabric a project's pieces are mostly cut from: its main fabric, if any. */
export function mainMaterial(materials: string[]): string {
  return materials.includes('main') ? 'main' : (materials[0] ?? 'main')
}

/** The Layout button, and behind it the cutting layout. */
export function LayoutDialog({ session }: { session: Session }) {
  useEngineState(session)
  const empty = session.engine.document().pieces.length === 0
  return (
    <DialogTrigger>
      <Button className="button" isDisabled={empty} aria-description={text.openHint}>
        <Scissors size={18} aria-hidden="true" />
        <span className="button-label minor">{text.open}</span>
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal wide">
          <Dialog className="dialog">
            {({ close }) => <LayoutForm session={session} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}

/** A length of fabric as sold: metres to the next 5 cm, or yards to the next eighth. */
export function fabricLength(mm: number, units: DisplayUnits): string {
  if (units === 'in') {
    const eighths = Math.ceil(mm / 25.4 / 36 / 0.125 - 1e-9)
    const whole = Math.floor(eighths / 8)
    const part = ['', '⅛', '¼', '⅜', '½', '⅝', '¾', '⅞'][eighths % 8]
    return `${whole > 0 || !part ? whole : ''}${part} yd`
  }
  return `${(Math.ceil(mm / 50 - 1e-9) * 0.05).toFixed(2)} m`
}

/** A fabric width as sold: whole centimetres, or inches. */
export function widthName(mm: number, units: DisplayUnits): string {
  return units === 'in' ? `${Math.round(mm / 25.4)}″` : `${Math.round(mm / 10)} cm`
}

function LayoutForm({ session, close }: { session: Session; close: () => void }) {
  useEngineState(session)
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const sold = WIDTHS[units === 'in' ? 'imperial' : 'metric']
  const materials = [...new Set(doc.pieces.flatMap((p) => p.cut.map((c) => c.material)))]
  const [material, setMaterial] = useState(() => mainMaterial(materials))
  const [choice, setChoice] = useState<Choice>(() => keptChoice(units))
  const choose = (change: Partial<Choice>) => {
    const next = { ...choice, ...change }
    setChoice(next)
    remember(KEPT, next)
  }
  const [layout, setLayout] = useState<Layout | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  // Laid out again after every change, in the IO worker.
  useEffect(() => {
    let current = true
    session.io
      .call('layOut', {
        pieces: doc.pieces,
        material,
        width: choice.width,
        folded: choice.folded,
        oneWay: choice.oneWay,
      })
      .then(
        (l) => {
          if (!current) return
          setLayout(l)
          setProblem(null)
        },
        (error: unknown) => {
          if (current)
            setProblem(text.failed(error instanceof Error ? error.message : String(error)))
        },
      )
    return () => {
      current = false
    }
  }, [session, doc.pieces, material, choice])

  const nameOf = (id: string) => doc.pieces.find((p) => p.id === id)?.name ?? id
  const preset = sold.find((w) => Math.abs(w - choice.width) < 1)

  return (
    <>
      <Heading slot="title">{text.heading}</Heading>
      <div className="layout-choices">
        {materials.length > 1 && (
          <Select
            className="field compact"
            selectedKey={material}
            onSelectionChange={(m) => setMaterial(String(m))}
          >
            <Label>{text.material}</Label>
            <Button className="select-button">
              <SelectValue>{() => materialName(material)}</SelectValue>
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            <Popover className="popover">
              <ListBox className="menu">
                {materials.map((m) => (
                  <ListBoxItem key={m} id={m} className="menu-item">
                    {materialName(m)}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>
        )}
        <Select
          className="field compact"
          selectedKey={preset === undefined ? 'other' : String(preset)}
          onSelectionChange={(w) => {
            if (w !== 'other') choose({ width: Number(w) })
            else choose({ width: Math.round(choice.width) + 1 })
          }}
        >
          <Label>{text.width}</Label>
          <Button className="select-button">
            <SelectValue />
            <ChevronDown size={16} aria-hidden="true" />
          </Button>
          <Popover className="popover">
            <ListBox className="menu">
              {sold.map((w) => (
                <ListBoxItem key={w} id={String(w)} className="menu-item">
                  {widthName(w, units)}
                </ListBoxItem>
              ))}
              <ListBoxItem id="other" className="menu-item">
                {text.otherWidth}
              </ListBoxItem>
            </ListBox>
          </Popover>
        </Select>
        {preset === undefined && (
          <LengthField
            label={text.ownWidth}
            value={choice.width}
            units={units}
            onChange={(width) => width > 0 && choose({ width })}
          />
        )}
        <Switch
          className="switch"
          isSelected={choice.folded}
          onChange={(folded) => choose({ folded })}
        >
          <span className="switch-track" aria-hidden="true" />
          {text.folded}
        </Switch>
        <Switch
          className="switch"
          isSelected={choice.oneWay}
          onChange={(oneWay) => choose({ oneWay })}
        >
          <span className="switch-track" aria-hidden="true" />
          {text.oneWay}
        </Switch>
      </div>
      {problem && <p className="measure-warning small">{problem}</p>}
      {layout && layout.placed.length > 0 && (
        <>
          <p className="layout-takes" role="status">
            {text.takes(
              fabricLength(layout.length, units),
              widthName(layout.width, units),
              materialName(material).toLowerCase(),
              layout.folded,
              Math.round(layout.used * 100),
            )}
          </p>
          <FabricPicture layout={layout} />
        </>
      )}
      {layout && layout.placed.length === 0 && <p className="muted">{text.nothing}</p>}
      {layout && layout.notes.length > 0 && (
        <ul className="notes">
          {layout.notes.map((n, i) => (
            <li key={i}>{text.note(n.kind, nameOf(n.piece))}</li>
          ))}
        </ul>
      )}
      <p className="muted small">{text.how}</p>
      <div className="dialog-actions">
        <Button className="button" onPress={close}>
          {t.actions.close}
        </Button>
      </div>
    </>
  )
}

/** The fabric, its fold or one selvage at the top, the pieces on it. */
function FabricPicture({ layout }: { layout: Layout }) {
  const across = layout.folded ? layout.width / 2 : layout.width
  const length = Math.max(layout.length, 1)
  const margin = 20
  const label = Math.max(14, Math.min(28, across / 25))
  return (
    <div className="layout-picture">
      <svg
        viewBox={`${-margin} ${-margin - label} ${length + 2 * margin} ${across + 2 * margin + 2 * label}`}
        role="img"
        aria-label={text.picture(layout.placed.length)}
      >
        <rect className="layout-fabric" x={0} y={0} width={length} height={across} />
        <text className="layout-edge" x={4} y={-6} fontSize={label}>
          {layout.folded ? text.fold : text.selvage}
        </text>
        <text className="layout-edge" x={4} y={across + label + 4} fontSize={label}>
          {text.selvage}
        </text>
        {layout.folded && <line className="layout-fold" x1={0} y1={0} x2={length} y2={0} />}
        {layout.placed.map((p, i) => (
          <PlacedPiece key={i} placed={p} label={label} />
        ))}
      </svg>
    </div>
  )
}

function PlacedPiece({ placed, label }: { placed: Placed; label: number }) {
  const points = placed.rings.flat()
  const xs = points.map((p) => p[0])
  const ys = points.map((p) => p[1])
  const [cx, cy] = [
    (Math.min(...xs) + Math.max(...xs)) / 2,
    (Math.min(...ys) + Math.max(...ys)) / 2,
  ]
  const [g0, g1] = placed.grain
  return (
    <g className={`layout-piece${placed.mirrored ? ' face-down' : ''}`}>
      {placed.rings.map((ring, k) => (
        <polygon key={k} points={ring.map((p) => `${p[0]},${p[1]}`).join(' ')} />
      ))}
      <line className="layout-grain" x1={g0[0]} y1={g0[1]} x2={g1[0]} y2={g1[1]} />
      <text x={cx} y={cy} fontSize={label} textAnchor="middle" dominantBaseline="middle">
        {placed.name}
      </text>
      {placed.mirrored && (
        <text x={cx} y={cy + label * 1.2} fontSize={label * 0.8} textAnchor="middle">
          {text.faceDown}
        </text>
      )}
    </g>
  )
}
