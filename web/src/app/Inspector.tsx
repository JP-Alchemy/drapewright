import type {
  Command,
  DisplayUnits,
  EdgeKind,
  EdgeMove,
  Piece,
  PieceView,
} from '@drapewright/engine'
import {
  ChevronRight,
  Minus,
  MoveVertical,
  Plus,
  RotateCcw,
  RotateCw,
  Trash2,
  X,
} from 'lucide-react'
import { type ReactNode, useEffect, useId, useRef, useState } from 'react'
import {
  Button,
  Disclosure,
  DisclosurePanel,
  Heading,
  Input,
  Label,
  Radio,
  RadioGroup,
  Switch,
  TextField,
} from 'react-aria-components'
import { remember, remembered } from '../platform/remember.ts'
import { useEngineState } from '../state/hooks.ts'
import { edgeLine } from '../state/edge-line.ts'
import type { EdgeRef, PartRef, Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { shownTurn } from '../state/straighten.ts'
import { formatLength, t } from '../strings/en.ts'
import { CornerChooser } from './CornerChooser.tsx'
import { LengthField } from './LengthField.tsx'
import { AlterSection, AlterStep, alterSummary } from './AlterSection.tsx'
import {
  CutSection,
  GrainSection,
  ManyCutSection,
  NotchSection,
  grainSummary,
} from './PieceDetails.tsx'
import { SeamSection } from './Seams.tsx'
import { TableSizes } from './TableSizes.tsx'
import { edgeLabel, sideLabel } from './labels.ts'
import { cornerSteps } from '../tools/corner-move.ts'

/**
 * A text field that reports its value when editing ends, not per keystroke.
 * Give it a `key` that changes with `value` so outside changes (undo) show.
 */
export function CommitTextField({
  label,
  value,
  placeholder,
  onCommit,
}: {
  label: string
  value: string
  placeholder?: string
  onCommit(v: string): void
}) {
  const [draft, setDraft] = useState(value)
  const commit = () => {
    const next = draft.trim()
    if (next !== value) onCommit(next)
  }
  return (
    <TextField
      className="field"
      value={draft}
      onChange={setDraft}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    >
      <Label>{label}</Label>
      <Input className="text-input" placeholder={placeholder} />
    </TextField>
  )
}

/** Which parts of the details are folded away, as last left on this device. */
const FOLDED_KEY = 'drapewright.details.folded'

function useFolded(id: string): [boolean, (folded: boolean) => void] {
  const [folded, setFolded] = useState(() => remembered<string[]>(FOLDED_KEY, []).includes(id))
  const set = (next: boolean) => {
    // Read afresh: other parts may have been folded since this one was drawn.
    const others = remembered<string[]>(FOLDED_KEY, []).filter((f) => f !== id)
    remember(FOLDED_KEY, next ? [...others, id] : others)
    setFolded(next)
  }
  return [folded, set]
}

/**
 * A part of the details that folds away, saying in a line what it holds
 * while folded; folded or not as it was last left.
 */
function DetailsPart({
  id,
  title,
  summary,
  children,
}: {
  id: string
  title: string
  summary: string
  children: ReactNode
}) {
  const [folded, setFolded] = useFolded(id)
  return (
    <Disclosure
      className="details-part"
      isExpanded={!folded}
      onExpandedChange={(open) => setFolded(!open)}
    >
      <Heading level={3} className="details-heading">
        <Button slot="trigger" className="details-trigger">
          <ChevronRight className="details-chevron" size={18} aria-hidden="true" />
          <span>{title}</span>
          {folded && <span className="details-summary muted small">{summary}</span>}
        </Button>
      </Heading>
      <DisclosurePanel className="details-panel">{children}</DisclosurePanel>
    </Disclosure>
  )
}

/** "Added · 1.5 cm", or "Included · 1.0 cm, hems 2.0 cm"; "Mixed" where they differ. */
function allowanceSummary(pieces: Piece[], units: DisplayUnits): string {
  const [first] = pieces
  if (!first) return ''
  const same = pieces.every(
    (p) =>
      p.allowance.mode === first.allowance.mode &&
      p.allowance.seam === first.allowance.seam &&
      (p.allowance.hem ?? p.allowance.seam) === (first.allowance.hem ?? first.allowance.seam),
  )
  if (!same) return t.inspector.mixed
  const { mode, seam, hem } = first.allowance
  return t.inspector.allowanceSummary(
    mode === 'included',
    formatLength(seam, units),
    hem !== null && hem !== seam ? formatLength(hem, units) : null,
  )
}

/**
 * The allowance of one piece, or of several chosen together: a value they
 * don't share shows as mixed, and a change applies to all of them (a run of
 * changes is one undo step).
 */
function AllowanceFields({ session, pieces }: { session: Session; pieces: Piece[] }) {
  const units = session.engine.document().settings.displayUnits
  const step = { coalesce: `allowance:${pieces.map((p) => p.id).join(',')}` }
  const change = (edit: (a: Piece['allowance']) => Piece['allowance']) => {
    for (const p of pieces) {
      session.dispatch({ type: 'setAllowance', piece: p.id, allowance: edit(p.allowance) }, step)
    }
  }
  const included = pieces.map((p) => p.allowance.mode === 'included')
  const all = included.every(Boolean)
  const some = !all && included.some(Boolean)
  const shared = (xs: number[]) => (xs.every((x) => x === xs[0]) ? (xs[0] ?? null) : null)
  return (
    <>
      <Switch
        className="switch"
        isSelected={all}
        onChange={(on) => change((a) => ({ ...a, mode: on ? 'included' : 'add' }))}
      >
        <span className="switch-track" aria-hidden="true" />
        {t.inspector.included}
      </Switch>
      <p className="muted small">
        {some ? t.inspector.someIncluded : all ? t.inspector.includedHint : t.inspector.addedHint}
      </p>
      <LengthField
        label={t.inspector.seamAllowance}
        value={shared(pieces.map((p) => p.allowance.seam))}
        units={units}
        onChange={(seam) => change((a) => ({ ...a, seam }))}
        onStep={(by) => change((a) => ({ ...a, seam: Math.max(0, a.seam + by) }))}
      />
      <LengthField
        label={t.inspector.hemAllowance}
        value={shared(pieces.map((p) => p.allowance.hem ?? p.allowance.seam))}
        units={units}
        onChange={(hem) => change((a) => ({ ...a, hem }))}
        onStep={(by) => change((a) => ({ ...a, hem: Math.max(0, (a.hem ?? a.seam) + by) }))}
      />
    </>
  )
}

/**
 * A turn in degrees, typed to a tenth or stepped a degree at a time; mixed
 * where pieces chosen together differ, and each stepped from its own.
 */
function AngleField({
  label,
  value,
  onStep,
  onSet,
}: {
  label: string
  value: number | null
  onStep(by: number): void
  onSet(degrees: number): void
}) {
  const id = useId()
  const hint = useId()
  const shown = value === null ? '' : String(value)
  const [draft, setDraft] = useState(shown)
  const commit = () => {
    const v = Number.parseFloat(draft.replace(',', '.').replace('°', ''))
    if (Number.isFinite(v) && draft !== shown) onSet(v)
    else setDraft(shown)
  }
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <div className="number-group">
        <Button
          className="step-button"
          aria-label={t.inspector.decrease(label)}
          onPress={() => onStep(-1)}
        >
          <Minus size={16} aria-hidden="true" />
        </Button>
        <input
          id={id}
          className="number-input"
          value={draft}
          placeholder={value === null ? t.inspector.mixed : undefined}
          inputMode="decimal"
          aria-describedby={hint}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === 'Enter' && commit()}
        />
        <Button
          className="step-button"
          aria-label={t.inspector.increase(label)}
          onPress={() => onStep(1)}
        >
          <Plus size={16} aria-hidden="true" />
        </Button>
      </div>
      <p id={hint} className="muted small">
        {t.inspector.turnAngleHint}
      </p>
    </div>
  )
}

/** "As drawn", "3.5°", or "Mixed" where they differ. */
function turnSummary(pieces: Piece[]): string {
  const turns = pieces.map((p) => shownTurn(p.placement.rotation))
  return turns.every((d) => d === turns[0])
    ? t.inspector.turnSummary(turns[0] ?? 0)
    : t.inspector.mixed
}

/**
 * How pieces lie on the table: straightened to their grainline, turned a
 * quarter at a time, or to any angle typed in. Never their shape.
 */
function TurnFields({ session, pieces }: { session: Session; pieces: Piece[] }) {
  const ids = pieces.map((p) => p.id)
  const turns = pieces.map((p) => shownTurn(p.placement.rotation))
  const shared = turns.every((d) => d === turns[0]) ? (turns[0] ?? 0) : null
  const label = t.inspector.turnAngle
  return (
    <>
      <Button className="button" onPress={() => session.straightenPieces(ids)}>
        <MoveVertical size={18} aria-hidden="true" />
        {t.inspector.straighten}
      </Button>
      <p className="muted small">
        {pieces.length > 1 ? t.inspector.straightenManyHint : t.inspector.straightenHint}
      </p>
      <div className="button-row">
        <Button className="button" onPress={() => session.turnPieces(ids, 90)}>
          <RotateCw size={18} aria-hidden="true" />
          {t.inspector.turn}
        </Button>
        <Button className="button" onPress={() => session.turnPieces(ids, -90)}>
          <RotateCcw size={18} aria-hidden="true" />
          {t.inspector.turnBack}
        </Button>
      </div>
      <AngleField
        key={shared ?? 'mixed'}
        label={label}
        value={shared}
        onStep={(by) => session.setTurn(ids, by, true)}
        onSet={(degrees) => session.setTurn(ids, degrees)}
      />
    </>
  )
}

/**
 * One piece: its name, how it is cut (first, as it is set for every piece
 * of a new pattern), then its allowance, grainline and alterations, each of
 * which folds away.
 */
function PieceSection({
  session,
  piece,
  view,
}: {
  session: Session
  piece: Piece
  view: PieceView
}) {
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const [x0, y0, x1, y1] = view.outlineBounds
  // A piece sewn to nothing yet, and lines drawn on another to sew it onto.
  const canSewOnto =
    !doc.seams.some((s) => s.a.piece === piece.id || s.b.piece === piece.id) &&
    doc.pieces.some((p) => p.id !== piece.id && p.lines.length > 0)
  return (
    <section className="panel-section inspector" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">{t.inspector.heading}</h2>
      <CommitTextField
        key={`${piece.id}:${piece.name}`}
        label={t.inspector.name}
        value={piece.name}
        onCommit={(name) =>
          name && session.dispatch({ type: 'renamePiece', piece: piece.id, name })
        }
      />
      <p className="muted small">
        {t.pieces.size(formatLength(x1 - x0, units), formatLength(y1 - y0, units))}
      </p>
      {canSewOnto && (
        <Button
          className="button"
          aria-description={t.seams.sewOntoHint}
          onPress={() => session.startSewingOnto(piece.id)}
        >
          {t.seams.sewOnto}
        </Button>
      )}
      <TableSizes session={session} chosen={[piece]} />
      <DetailsPart id="turn" title={t.inspector.turnHeading} summary={turnSummary([piece])}>
        <TurnFields session={session} pieces={[piece]} />
      </DetailsPart>
      <CutSection session={session} piece={piece} />
      <DetailsPart
        id="allowance"
        title={t.inspector.allowance}
        summary={allowanceSummary([piece], units)}
      >
        <AllowanceFields session={session} pieces={[piece]} />
      </DetailsPart>
      <DetailsPart id="grain" title={t.grain.heading} summary={grainSummary(piece)}>
        <GrainSection session={session} piece={piece} view={view} />
      </DetailsPart>
      <DetailsPart id="alter" title={t.alter.heading} summary={alterSummary(piece)}>
        <AlterSection session={session} piece={piece} view={view} />
      </DetailsPart>
      {view.allowanceNotes.length + view.issues.length > 0 && (
        <ul className="notes">
          {view.allowanceNotes.map((n, i) => (
            <li key={`n${i}`}>{t.allowanceNote(n, (id) => edgeLabel(piece, id))}</li>
          ))}
          {view.issues.map((issue, i) => (
            <li key={`i${i}`}>{issue.message}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** Several pieces chosen together: how they are cut and their allowance, set for all at once. */
function ManyPiecesSection({ session, pieces }: { session: Session; pieces: Piece[] }) {
  const units = session.engine.document().settings.displayUnits
  return (
    <section className="panel-section inspector" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">{t.inspector.manyHeading(pieces.length)}</h2>
      <p className="muted small">{t.inspector.manyHint(pieces.map((p) => p.name))}</p>
      <div className="button-row">
        <Button className="button danger" onPress={() => session.deleteSelection()}>
          <Trash2 size={18} aria-hidden="true" />
          {t.menu.deletePieces(pieces.length)}
        </Button>
      </div>
      <TableSizes session={session} chosen={pieces} />
      <DetailsPart id="turn" title={t.inspector.turnHeading} summary={turnSummary(pieces)}>
        <TurnFields session={session} pieces={pieces} />
      </DetailsPart>
      <ManyCutSection session={session} pieces={pieces} />
      <DetailsPart
        id="allowance"
        title={t.inspector.allowance}
        summary={allowanceSummary(pieces, units)}
      >
        <AllowanceFields session={session} pieces={pieces} />
      </DetailsPart>
    </section>
  )
}

function EdgeSection({
  session,
  piece,
  view,
  edgeId,
}: {
  session: Session
  piece: Piece
  view: PieceView
  edgeId: string
}) {
  const units = session.engine.document().settings.displayUnits
  const edge = piece.outline.edges.find((e) => e.id === edgeId)
  const ev = view.edges.find((e) => e.id === edgeId)
  if (!edge || !ev) return null
  const ref = { piece: piece.id, edge: edgeId }
  const doc = session.engine.document()
  const here = (side: { piece: string; edge: string }) =>
    side.piece === piece.id && side.edge === edgeId
  const sewn = doc.seams
    .map((seam, i) => ({ seam, i }))
    .filter(({ seam }) => here(seam.a) || here(seam.b))
  // The line round the garment it is part of.
  const line = edgeLine(doc, ref)
  // Its moves, as dragged by the handles beside its length.
  const moves = piece.alterations.filter(
    (a) => a.change.type === 'moveEdge' && a.change.edge === edgeId,
  )
  return (
    <section className="panel-section inspector" aria-labelledby="edge-heading">
      <h2 id="edge-heading">{t.inspector.edge}</h2>
      {sewn.map(({ seam, i }) => (
        <Button
          key={seam.id}
          className="link-button"
          onPress={() => session.selectSeam(seam.id, piece.id)}
        >
          {t.seams.sewnTo(sideLabel(doc, here(seam.a) && !seam.a.mirror ? seam.b : seam.a))} (
          {t.seams.seam(i + 1, seam.name)})
        </Button>
      ))}
      <Button className="button" onPress={() => session.startSewing(piece.id, edgeId)}>
        {t.seams.sewTo}
      </Button>
      <CommitTextField
        key={`${edgeId}:${edge.name}`}
        label={t.inspector.edgeName}
        value={edge.name}
        placeholder={t.inspector.edgeNamePlaceholder}
        onCommit={(name) => session.dispatch({ type: 'renameEdge', ...ref, name })}
      />
      <p className="muted small">{t.inspector.edgeLength(formatLength(ev.length, units))}</p>
      {line.length > 1 && (
        <Button className="link-button" onPress={() => session.chooseLine(ref)}>
          {t.edges.chooseLine(line.length)}
        </Button>
      )}
      {moves.map((a) => (
        <AlterStep key={a.id} session={session} piece={piece} view={view} alteration={a} />
      ))}
      {moves.length === 0 && <p className="muted small">{t.edgeMove.hint}</p>}
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        value={edge.kind}
        onChange={(kind) =>
          session.dispatch({ type: 'setEdgeKind', ...ref, kind: kind as EdgeKind })
        }
      >
        <Label className="group-label">{t.inspector.kind}</Label>
        <div className="segments">
          {(['seam', 'hem', 'fold'] as const).map((k) => (
            <Radio key={k} value={k} className="segment">
              {t.inspector.kinds[k]}
            </Radio>
          ))}
        </div>
        <p className="muted small">{t.inspector.kindHints[edge.kind]}</p>
      </RadioGroup>
      {edge.kind === 'fold' ? (
        <p className="muted small">{t.inspector.foldNoAllowance}</p>
      ) : (
        <>
          <LengthField
            label={t.inspector.edgeAllowance}
            value={ev.allowance}
            units={units}
            onChange={(allowance) =>
              session.dispatch(
                { type: 'setEdgeAllowance', ...ref, allowance },
                { coalesce: `edge-allowance:${edgeId}` },
              )
            }
          />
          {edge.allowance !== null && (
            <Button
              className="link-button"
              onPress={() =>
                session.dispatch({ type: 'setEdgeAllowance', ...ref, allowance: null })
              }
            >
              {t.inspector.usePieceAllowance}
            </Button>
          )}
        </>
      )}
    </section>
  )
}

/** How far an edge's last step moving it `mode` moves it, mm: 0 with none. */
function movedBy(piece: Piece | undefined, edge: string, mode: EdgeMove): number {
  const step = piece?.alterations.findLast(
    (a) => a.change.type === 'moveEdge' && a.change.edge === edge && a.change.mode === mode,
  )
  return step?.change.type === 'moveEdge' ? step.change.by : 0
}

/**
 * Edges chosen together (a skirt's hem all round, say): each named, to be
 * left out; and how far they are all moved out, extended or extruded, as a
 * step on each one's piece.
 */
function EdgesSection({ session, edges }: { session: Session; edges: EdgeRef[] }) {
  const [mode, setMode] = useState<EdgeMove>('extend')
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const pieceOf = (id: string) => doc.pieces.find((p) => p.id === id)
  const amounts = edges.map((e) => movedBy(pieceOf(e.piece), e.edge, mode))
  const same = amounts.every((a) => Math.abs(a - amounts[0]!) < 0.05)
  /** Each edge moved `to(its amount)`, as one undo step: its step changed, added or taken away. */
  const move = (to: (was: number) => number) => {
    const commands: Command[] = edges.flatMap((e, i): Command[] => {
      const piece = pieceOf(e.piece)
      const step = piece?.alterations.findLast(
        (a) => a.change.type === 'moveEdge' && a.change.edge === e.edge && a.change.mode === mode,
      )
      const by = Math.round(to(amounts[i]!) * 10) / 10
      if (!piece) return []
      if (step && by === 0) {
        return [{ type: 'deleteAlteration', piece: piece.id, alteration: step.id }]
      }
      const change = { type: 'moveEdge' as const, edge: e.edge, by, mode }
      if (step) {
        return [{ type: 'setAlteration', piece: piece.id, alteration: { ...step, change } }]
      }
      return by === 0
        ? []
        : [{ type: 'addAlteration', piece: piece.id, alteration: { id: '', on: true, change } }]
    })
    if (commands.length > 0) session.dispatchAll(commands)
  }
  return (
    <section className="panel-section inspector" aria-labelledby="edges-heading">
      <h2 id="edges-heading">{t.edges.heading(edges.length)}</h2>
      <ul className="chosen-edges">
        {edges.map((e) => {
          const piece = pieceOf(e.piece)
          const name = piece ? `${piece.name} · ${edgeLabel(piece, e.edge)}` : e.edge
          return (
            <li key={`${e.piece} ${e.edge}`}>
              <span>{name}</span>
              <Button
                className="icon-button"
                aria-label={t.edges.leaveOut(name)}
                onPress={() => session.toggleEdge(e)}
              >
                <X size={16} aria-hidden="true" />
              </Button>
            </li>
          )
        })}
      </ul>
      <p className="muted small">{t.edges.hint}</p>
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        aria-label={t.alter.sides}
        value={mode}
        onChange={(m) => setMode(m as EdgeMove)}
      >
        <Label className="group-label">{t.alter.sides}</Label>
        <div className="segments">
          <Radio value="extend" className="segment">
            {t.alter.extend}
          </Radio>
          <Radio value="extrude" className="segment">
            {t.alter.extrude}
          </Radio>
        </div>
      </RadioGroup>
      <LengthField
        label={t.edges.by}
        value={same ? amounts[0]! : null}
        units={units}
        min={-1000}
        description={t.edges.byHint}
        onChange={(mm) => move(() => mm)}
        onStep={(mm) => move((was) => was + mm)}
      />
    </section>
  )
}

function CornerSection({
  session,
  piece,
  view,
  edgeId,
}: {
  session: Session
  piece: Piece
  view: PieceView
  edgeId: string
}) {
  const i = piece.outline.edges.findIndex((e) => e.id === edgeId)
  if (i < 0) return null
  const next = piece.outline.edges[(i + 1) % piece.outline.edges.length]!
  return (
    <section className="panel-section inspector" aria-labelledby="corner-heading">
      <h2 id="corner-heading">{t.corner.sectionHeading}</h2>
      <p className="muted small">
        {t.corner.between(edgeLabel(piece, edgeId), edgeLabel(piece, next.id))}
      </p>
      {cornerSteps(piece, edgeId).map((a) => (
        <AlterStep key={a.id} session={session} piece={piece} view={view} alteration={a} />
      ))}
      {cornerSteps(piece, edgeId).length === 0 && (
        <p className="muted small">{t.cornerMove.hint}</p>
      )}
      <CornerChooser
        session={session}
        piece={piece}
        edgeId={edgeId}
        left={edgeLabel(piece, edgeId)}
        right={edgeLabel(piece, next.id)}
      />
      {piece.outline.edges.length > 1 && (
        <Button
          className="button"
          onPress={() => session.dispatch({ type: 'joinEdges', piece: piece.id, edge: edgeId })}
        >
          {t.corner.remove}
        </Button>
      )}
    </section>
  )
}

/**
 * Details of the selected piece, edge or corner, or of the seam picked on
 * it, editable in sewing terms; or of several pieces, set together. Each
 * new choice shows from the top.
 */
export function Inspector({ session }: { session: Session }) {
  const { selection, part, seam, moreEdges } = useStore(session.ui)
  useEngineState(session)
  const top = useRef<HTMLDivElement>(null)
  const chosen = [...selection].sort().join(',')
  useEffect(() => {
    top.current?.closest('.side-details')?.scrollTo({ top: 0 })
  }, [chosen, seam])

  // In the pieces' order, whichever was chosen first.
  const scene = session.scene().filter((sp) => selection.has(sp.piece.id))
  let body: ReactNode
  if (scene.length === 0) {
    body = (
      <section className="panel-section">
        <p className="muted small">{t.inspector.nothing}</p>
      </section>
    )
  } else if (moreEdges.length > 0) {
    // Several edges chosen together, on one piece or several.
    const [only] = scene
    body = (
      <>
        <EdgesSection session={session} edges={session.chosenEdges()} />
        {scene.length > 1 || !only ? (
          <ManyPiecesSection session={session} pieces={scene.map((sp) => sp.piece)} />
        ) : (
          <PieceSection session={session} piece={only.piece} view={only.view} />
        )}
      </>
    )
  } else if (scene.length > 1) {
    body = <ManyPiecesSection session={session} pieces={scene.map((sp) => sp.piece)} />
  } else {
    const { piece, view } = scene[0]!
    const own: PartRef | null = part?.piece === piece.id ? part : null
    const picked = session.engine.document().seams.find((s) => s.id === seam)
    body = (
      <>
        {picked && <SeamSection session={session} seam={picked} />}
        {own?.kind === 'corner' && (
          <CornerSection session={session} piece={piece} view={view} edgeId={own.edge} />
        )}
        {own?.kind === 'edge' && (
          <EdgeSection session={session} piece={piece} view={view} edgeId={own.edge} />
        )}
        {own?.kind === 'notch' && (
          <NotchSection session={session} piece={piece} view={view} notchId={own.notch} />
        )}
        <PieceSection session={session} piece={piece} view={view} />
      </>
    )
  }
  return <div ref={top}>{body}</div>
}
