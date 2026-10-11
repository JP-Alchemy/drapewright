// A piece's alterations in the inspector: changes kept as steps on the
// piece as drawn, each switched on or off, changed or taken away.

import type { AlterLine, Alteration, Piece, PieceView } from '@drapewright/engine'
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
  const add = () => {
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
      <Button className="button" onPress={add}>
        <Plus size={18} aria-hidden="true" />
        {t.alter.lengthen}
      </Button>
    </div>
  )
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
  const longer = change.by >= 0
  const amount = Math.abs(change.by)
  const set = (next: Alteration, coalesce?: string) =>
    session.dispatch(
      { type: 'setAlteration', piece: piece.id, alteration: next },
      // Stepping an amount up and down is one undo step.
      coalesce ? { coalesce } : undefined,
    )
  const own = piece.lines.filter((l) => l.kind === 'lengthenShorten')
  const where = change.line.type === 'own' ? change.line.line : ACROSS
  const said = t.alter.step(longer, formatLength(amount, units))
  const down = change.line.type === 'through' ? change.line.from[1] - view.outlineBounds[1] : 0
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
        aria-label={t.alter.way}
        value={longer ? 'longer' : 'shorter'}
        onChange={(way) =>
          set({ ...alteration, change: { ...change, by: (way === 'longer' ? 1 : -1) * amount } })
        }
      >
        <div className="segments">
          <Radio value="longer" className="segment">
            {t.alter.longer}
          </Radio>
          <Radio value="shorter" className="segment">
            {t.alter.shorter}
          </Radio>
        </div>
      </RadioGroup>
      <LengthField
        label={t.alter.by}
        value={amount}
        units={units}
        onChange={(mm) =>
          set(
            { ...alteration, change: { ...change, by: (longer ? 1 : -1) * mm } },
            `alter:${alteration.id}`,
          )
        }
      />
      {own.length > 0 && (
        <Select
          className="field"
          selectedKey={where}
          onSelectionChange={(key) =>
            set({
              ...alteration,
              change: {
                ...change,
                line:
                  key === ACROSS ? across(view, middle(view)) : { type: 'own', line: String(key) },
              },
            })
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
          onChange={(mm) =>
            set(
              { ...alteration, change: { ...change, line: across(view, mm) } },
              `at:${alteration.id}`,
            )
          }
        />
      )}
    </div>
  )
}
