// A piece's alterations in the inspector: changes kept as steps on the
// piece as drawn, each switched on or off, changed or taken away.

import type { AlterLine, Alteration, Change, Piece, PieceView } from '@drapewright/engine'
import { ChevronDown, Plus, Trash2 } from 'lucide-react'
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Switch,
} from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import { LengthField } from './LengthField.tsx'

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
  const lengthen = () => {
    const line: AlterLine = own[0] ? { type: 'own', line: own[0].id } : across(view, middle(view))
    session.dispatch({
      type: 'addAlteration',
      piece: piece.id,
      alteration: { id: '', on: true, change: { type: 'lengthen', line, by: 10 } },
    })
  }
  return (
    <div className="subsection">
      <h3>{t.alter.heading}</h3>
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
      <div className="alter-add">
        <Button className="button" onPress={lengthen}>
          <Plus size={18} aria-hidden="true" />
          {t.alter.lengthen}
        </Button>
        <Button className="button" onPress={() => session.startPicking(piece.id, 'spread')}>
          <Plus size={18} aria-hidden="true" />
          {t.alter.spread}
        </Button>
        <Button className="button" onPress={() => session.startPicking(piece.id, 'bust')}>
          <Plus size={18} aria-hidden="true" />
          {t.alter.bust}
        </Button>
      </div>
    </div>
  )
}

/** What a step says it does, and the two ways it can go (more, or less). */
function wording(change: Change, by: string) {
  const more = change.by >= 0
  switch (change.type) {
    case 'lengthen':
      return {
        said: t.alter.step(more, by),
        way: t.alter.way,
        ways: [t.alter.longer, t.alter.shorter],
      }
    case 'spread':
      return {
        said: t.alter.spreadStep(more, by),
        way: t.alter.spreadWay,
        ways: [t.alter.open, t.alter.close],
      }
    case 'fullBust':
      return {
        said: t.alter.bustStep(more, by),
        way: t.alter.bustWay,
        ways: [t.alter.full, t.alter.small],
      }
  }
}

function AlterStep({
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
  const more = change.by >= 0
  const amount = Math.abs(change.by)
  const set = (next: Alteration, coalesce?: string) =>
    session.dispatch(
      { type: 'setAlteration', piece: piece.id, alteration: next },
      // Stepping an amount up and down is one undo step.
      coalesce ? { coalesce } : undefined,
    )
  const withChange = (next: Change, coalesce?: string) =>
    set({ ...alteration, change: next }, coalesce)
  const { said, way, ways } = wording(change, formatLength(amount, units))
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
        description={change.type === 'fullBust' ? t.alter.bustHint : undefined}
        onChange={(mm) =>
          withChange({ ...change, by: (more ? 1 : -1) * mm }, `alter:${alteration.id}`)
        }
      />
      {change.type === 'lengthen' && (
        <LineChoice session={session} piece={piece} view={view} alteration={alteration} />
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
