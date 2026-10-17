// A piece's alterations in the inspector: changes kept as steps on the
// piece as drawn, each switched on or off, changed or taken away.

import type {
  AlterLine,
  Alteration,
  Change,
  DisplayUnits,
  EdgeMove,
  Piece,
  PieceView,
} from '@drapewright/engine'
import { ChevronDown, Plus, Trash2 } from 'lucide-react'
import {
  Button,
  Group,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Menu,
  MenuItem,
  MenuTrigger,
  NumberField,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Switch,
  Text,
} from 'react-aria-components'
import type { PickFor, Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import { edgeLabel } from './labels.ts'
import { LengthField } from './LengthField.tsx'
import { cornerMoves } from '../tools/corner-move.ts'

/** The choice of a line straight across the piece, among its own lines. */
const ACROSS = 'across'

/**
 * A line straight across the piece, `down` mm from its top. Its top stays
 * where it is when the piece is lengthened below the line.
 */
function across(view: PieceView, down: number): AlterLine {
  const [x0, y0, x1] = view.outlineBounds
  const y = y0 + down
  return { type: 'through', from: [x0 - 20, y], to: [x1 + 20, y] }
}

/** Halfway down the piece. */
function middle(view: PieceView): number {
  const [, y0, , y1] = view.outlineBounds
  return (y1 - y0) / 2
}

export function AlterSection({
  session,
  piece,
  view,
}: {
  session: Session
  piece: Piece
  view: PieceView
}) {
  const own = piece.lines.filter((l) => l.kind === 'lengthenShorten')
  const stretch = () =>
    session.dispatch({
      type: 'addAlteration',
      piece: piece.id,
      alteration: {
        id: '',
        on: true,
        change: { type: 'stretch', pattern: 20, fabric: 20, bothWays: false },
      },
    })
  const lengthen = () => {
    const line: AlterLine = own[0] ? { type: 'own', line: own[0].id } : across(view, middle(view))
    session.dispatch({
      type: 'addAlteration',
      piece: piece.id,
      alteration: { id: '', on: true, change: { type: 'lengthen', line, by: 10 } },
    })
  }
  return (
    <>
      {piece.alterations.length === 0 ? (
        <p className="muted small">{t.alter.none}</p>
      ) : (
        <>
          {piece.alterations.map((a) => (
            <AlterStep key={a.id} session={session} piece={piece} view={view} alteration={a} />
          ))}
          <p className="muted small">{t.alter.asDrawn}</p>
        </>
      )}
      <MenuTrigger>
        <Button className="button">
          <Plus size={18} aria-hidden="true" />
          {t.alter.add}
        </Button>
        <Popover className="popover">
          <Menu
            className="menu"
            onAction={(key) => {
              if (key === 'lengthen') lengthen()
              else if (key === 'stretch') stretch()
              else session.startPicking(piece.id, key as PickFor)
            }}
          >
            {KINDS.filter((k) => k !== 'pivot' || piece.darts.length > 0).map((k) => (
              <MenuItem
                key={k}
                id={k}
                className="menu-item described"
                textValue={t.alter.kinds[k]![0]}
              >
                <span>{t.alter.kinds[k]![0]}</span>
                <span className="muted small">{t.alter.kinds[k]![1]}</span>
              </MenuItem>
            ))}
          </Menu>
        </Popover>
      </MenuTrigger>
    </>
  )
}

/** "None", "1 step" or "3 steps". */
export function alterSummary(piece: Piece): string {
  return t.alter.summary(piece.alterations.length)
}

/** The kinds of step, in the order offered. */
const KINDS = [
  'lengthen',
  'moveEdge',
  'spread',
  'bust',
  'shoulder',
  'swayback',
  'bustSide',
  'waist',
  'hip',
  'pivot',
  'trueUp',
  'stretch',
] as const

/** What a step that changes by an amount says it does, and the two ways it can go (more, or less). */
function wording(
  change: Exclude<Change, { type: 'pivot' | 'trueUp' | 'stretch' | 'moveCorner' }>,
  by: string,
  edge?: string,
) {
  const more = change.by >= 0
  switch (change.type) {
    case 'swayback':
      return {
        said: t.alter.swaybackStep(more, by),
        way: t.alter.swaybackWay,
        ways: [t.alter.takeOut, t.alter.letOut],
        hint: t.alter.swaybackHint,
      }
    case 'shoulder':
      return {
        said: t.alter.shoulderStep(more, by),
        way: t.alter.shoulderWay,
        ways: [t.alter.broader, t.alter.narrower],
        hint: t.alter.shoulderHint,
      }
    case 'bustSide':
    case 'waist':
    case 'hip':
      return {
        said: {
          bustSide: t.alter.bustSideStep,
          waist: t.alter.waistStep,
          hip: t.alter.hipStep,
        }[change.type](more, by),
        way: t.alter.sideWay,
        ways: [t.alter.letOut, t.alter.takeOut],
        hint: t.alter.sideHint,
      }
    case 'moveEdge':
      return {
        said: t.alter.moveStep(edge ?? '', change.mode, change.by, by),
        way: t.alter.moveWay,
        ways: [t.alter.out, t.alter.in],
        hint: undefined,
      }
    case 'lengthen':
      return {
        said: t.alter.step(more, by),
        way: t.alter.way,
        ways: [t.alter.longer, t.alter.shorter],
        hint: undefined,
      }
    case 'spread':
      return {
        said: t.alter.spreadStep(more, by),
        way: t.alter.spreadWay,
        ways: [t.alter.open, t.alter.close],
        hint: undefined,
      }
    case 'fullBust':
      return {
        said: t.alter.bustStep(more, by),
        way: t.alter.bustWay,
        ways: [t.alter.full, t.alter.small],
        hint: t.alter.bustHint,
      }
  }
}

/** One step: switched on or off, changed, or taken away. */
export function AlterStep({
  session,
  piece,
  view,
  alteration,
}: {
  session: Session
  piece: Piece
  view: PieceView
  alteration: Alteration
}) {
  const units = session.engine.document().settings.displayUnits
  const { change } = alteration
  const set = (next: Alteration, coalesce?: string) =>
    session.dispatch(
      { type: 'setAlteration', piece: piece.id, alteration: next },
      // Stepping an amount up and down is one undo step.
      coalesce ? { coalesce } : undefined,
    )
  const withChange = (next: Change, coalesce?: string) =>
    set({ ...alteration, change: next }, coalesce)
  const said =
    change.type === 'pivot'
      ? t.alter.pivotStep(change.share)
      : change.type === 'trueUp'
        ? t.alter.trueStep(edgeLabel(piece, change.edge), change.straight)
        : change.type === 'stretch'
          ? t.alter.stretchStep(
              Math.round((stretchFactor(change.pattern, change.fabric) - 1) * 100),
              change.bothWays,
            )
          : change.type === 'moveCorner'
            ? cornerSaid(piece, change, units)
            : wording(
                change,
                formatLength(Math.abs(change.by), units),
                change.type === 'moveEdge' ? edgeLabel(piece, change.edge) : undefined,
              ).said
  return (
    <div className="alter-step">
      <div className="alter-step-head">
        <Switch
          className="switch"
          isSelected={alteration.on}
          onChange={(on) => set({ ...alteration, on })}
        >
          <span className="switch-track" aria-hidden="true" />
          {said}
        </Switch>
        <Button
          className="icon-button"
          aria-label={t.alter.remove(said)}
          onPress={() =>
            session.dispatch({
              type: 'deleteAlteration',
              piece: piece.id,
              alteration: alteration.id,
            })
          }
        >
          <Trash2 size={18} aria-hidden="true" />
        </Button>
      </div>
      {change.type === 'pivot' ? (
        <RadioGroup
          className="segmented"
          orientation="horizontal"
          aria-label={t.alter.share}
          value={change.share >= 1 ? 'all' : 'half'}
          onChange={(w) => withChange({ ...change, share: w === 'all' ? 1 : 0.5 })}
        >
          <div className="segments">
            <Radio value="all" className="segment">
              {t.alter.all}
            </Radio>
            <Radio value="half" className="segment">
              {t.alter.half}
            </Radio>
          </div>
          <p className="muted small">{t.alter.pivotHint}</p>
        </RadioGroup>
      ) : change.type === 'trueUp' ? (
        <RadioGroup
          className="segmented"
          orientation="horizontal"
          aria-label={t.alter.trueWay}
          value={change.straight ? 'straight' : 'smooth'}
          onChange={(w) => withChange({ ...change, straight: w === 'straight' })}
        >
          <div className="segments">
            <Radio value="straight" className="segment">
              {t.alter.straight}
            </Radio>
            <Radio value="smooth" className="segment">
              {t.alter.smooth}
            </Radio>
          </div>
          <p className="muted small">{t.alter.trueHint}</p>
        </RadioGroup>
      ) : change.type === 'stretch' ? (
        <StretchFields
          session={session}
          piece={piece}
          alteration={alteration}
          change={change}
          withChange={withChange}
        />
      ) : change.type === 'moveCorner' ? (
        <>
          <LengthField
            label={t.alter.cornerRight}
            value={change.by[0]}
            units={units}
            min={-MOST_MOVED}
            onChange={(mm) =>
              withChange({ ...change, by: [mm, change.by[1]] }, `corner:${alteration.id}`)
            }
          />
          <LengthField
            label={t.alter.cornerDownBy}
            value={change.by[1]}
            units={units}
            min={-MOST_MOVED}
            description={t.alter.cornerHint}
            onChange={(mm) =>
              withChange({ ...change, by: [change.by[0], mm] }, `corner:${alteration.id}`)
            }
          />
        </>
      ) : (
        <Amount change={change} units={units} withChange={withChange} id={alteration.id} />
      )}
      {change.type === 'lengthen' && (
        <LineChoice session={session} piece={piece} view={view} alteration={alteration} />
      )}
      {change.type === 'moveEdge' && (
        <RadioGroup
          className="segmented"
          orientation="horizontal"
          aria-label={t.alter.sides}
          value={change.mode}
          onChange={(mode) => withChange({ ...change, mode: mode as EdgeMove })}
        >
          <Label>{t.alter.sides}</Label>
          <div className="segments">
            <Radio value="extend" className="segment">
              {t.alter.extend}
            </Radio>
            <Radio value="extrude" className="segment">
              {t.alter.extrude}
            </Radio>
          </div>
          <p className="muted small">{t.alter.moveHint(change.by >= 0)}</p>
        </RadioGroup>
      )}
      {change.type === 'spread' && (
        <Button
          className="link-button"
          onPress={() =>
            withChange({ ...change, turns: change.turns === 'right' ? 'left' : 'right' })
          }
        >
          {t.alter.otherSide}
        </Button>
      )}
    </div>
  )
}

/** The most a corner is moved either way, mm (as the engine allows). */
const MOST_MOVED = 1000

/** "Corner of the hem and edge 2 moved 2.0 cm right and 1.0 cm up". */
export function cornerSaid(
  piece: Piece,
  change: Extract<Change, { type: 'moveCorner' }>,
  units: DisplayUnits,
): string {
  const edges = piece.outline.edges
  const k = edges.findIndex((e) => e.id === change.edge)
  const i = change.start ? (k + edges.length - 1) % edges.length : k
  const [a, b] = [edges[i]?.id ?? change.edge, edges[(i + 1) % edges.length]?.id ?? change.edge]
  const moves = cornerMoves(change.by, units)
  return t.alter.cornerStep(edgeLabel(piece, a), edgeLabel(piece, b), moves)
}

/** As the engine has it: a piece made for `pattern` % stretch, cut from a fabric of `fabric` %. */
export function stretchFactor(pattern: number, fabric: number): number {
  return (1 + Math.max(pattern, 0) / 100) / (1 + Math.max(fabric, 0) / 100)
}

/**
 * A fabric stretch step: how far the pattern's fabric and this one
 * stretch, and whether both ways; and the same on every piece cut from
 * this fabric.
 */
function StretchFields({
  session,
  piece,
  alteration,
  change,
  withChange,
}: {
  session: Session
  piece: Piece
  alteration: Alteration
  change: Extract<Change, { type: 'stretch' }>
  withChange: (next: Change, coalesce?: string) => void
}) {
  const doc = session.engine.document()
  // A piece without cutting instructions is cut once from the main fabric.
  const materials = (p: Piece) => (p.cut.length > 0 ? p.cut.map((c) => c.material) : ['main'])
  const material = materials(piece)[0]
  // The other pieces cut from this fabric without a stretch step.
  const others = doc.pieces.filter(
    (p) =>
      p.id !== piece.id &&
      materials(p).includes(material!) &&
      !p.alterations.some((a) => a.change.type === 'stretch'),
  )
  const percent = (label: string, value: number, onChange: (v: number) => void, hint?: string) => (
    <NumberField
      className="field"
      value={value}
      minValue={0}
      maxValue={300}
      formatOptions={{ maximumFractionDigits: 0 }}
      onChange={(v) => Number.isFinite(v) && onChange(v)}
    >
      <Label>{label}</Label>
      <Group className="number-group with-unit">
        <Input className="number-input" />
        <span className="number-unit">%</span>
      </Group>
      {hint && (
        <Text slot="description" className="muted small">
          {hint}
        </Text>
      )}
    </NumberField>
  )
  return (
    <>
      {percent(
        t.alter.patternStretch,
        change.pattern,
        (pattern) => withChange({ ...change, pattern }, `alter:${alteration.id}`),
        t.alter.patternStretchHint,
      )}
      {percent(
        t.alter.fabricStretch,
        change.fabric,
        (fabric) => withChange({ ...change, fabric }, `alter:${alteration.id}`),
        t.alter.fabricStretchHint,
      )}
      <Switch
        className="switch"
        isSelected={change.bothWays}
        onChange={(bothWays) => withChange({ ...change, bothWays })}
      >
        <span className="switch-track" aria-hidden="true" />
        {t.alter.bothWays}
      </Switch>
      {others.length > 0 && (
        <Button
          className="link-button"
          onPress={() =>
            session.alterEach(
              others.map((p) => p.id),
              change,
            )
          }
        >
          {t.alter.everyPiece(others.length)}
        </Button>
      )}
    </>
  )
}

/** How much a step changes the piece, and which way. */
function Amount({
  change,
  units,
  withChange,
  id,
}: {
  change: Exclude<Change, { type: 'pivot' | 'trueUp' | 'stretch' | 'moveCorner' }>
  units: DisplayUnits
  withChange: (next: Change, coalesce?: string) => void
  id: string
}) {
  const more = change.by >= 0
  const amount = Math.abs(change.by)
  const { way, ways, hint } = wording(change, formatLength(amount, units))
  return (
    <>
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        aria-label={way}
        value={more ? 'more' : 'less'}
        onChange={(w) => withChange({ ...change, by: (w === 'more' ? 1 : -1) * amount })}
      >
        <div className="segments">
          <Radio value="more" className="segment">
            {ways[0]}
          </Radio>
          <Radio value="less" className="segment">
            {ways[1]}
          </Radio>
        </div>
      </RadioGroup>
      <LengthField
        label={t.alter.by}
        value={amount}
        units={units}
        description={hint}
        onChange={(mm) => withChange({ ...change, by: (more ? 1 : -1) * mm }, `alter:${id}`)}
      />
    </>
  )
}

/** Where a lengthening is: one of the piece's own lines, or straight across it. */
function LineChoice({
  session,
  piece,
  view,
  alteration,
}: {
  session: Session
  piece: Piece
  view: PieceView
  alteration: Alteration
}) {
  const { change } = alteration
  if (change.type !== 'lengthen') return null
  const units = session.engine.document().settings.displayUnits
  const own = piece.lines.filter((l) => l.kind === 'lengthenShorten')
  const where = change.line.type === 'own' ? change.line.line : ACROSS
  const down = change.line.type === 'through' ? change.line.from[1] - view.outlineBounds[1] : 0
  const setLine = (line: AlterLine, coalesce?: string) =>
    session.dispatch(
      {
        type: 'setAlteration',
        piece: piece.id,
        alteration: { ...alteration, change: { ...change, line } },
      },
      coalesce ? { coalesce } : undefined,
    )
  return (
    <>
      {own.length > 0 && (
        <Select
          className="field"
          selectedKey={where}
          onSelectionChange={(key) =>
            setLine(
              key === ACROSS ? across(view, middle(view)) : { type: 'own', line: String(key) },
            )
          }
        >
          <Label>{t.alter.where}</Label>
          <Button className="select-button">
            <SelectValue />
            <ChevronDown size={16} aria-hidden="true" />
          </Button>
          <Popover className="popover">
            <ListBox className="menu">
              {own.map((l, i) => (
                <ListBoxItem key={l.id} id={l.id} className="menu-item">
                  {t.alter.ownLine(i + 1, own.length)}
                </ListBoxItem>
              ))}
              <ListBoxItem id={ACROSS} className="menu-item">
                {t.alter.across}
              </ListBoxItem>
            </ListBox>
          </Popover>
        </Select>
      )}
      {change.line.type === 'through' && (
        <LengthField
          label={t.alter.fromTop}
          value={down}
          units={units}
          onChange={(mm) => setLine(across(view, mm), `at:${alteration.id}`)}
        />
      )}
    </>
  )
}
