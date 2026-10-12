import type { DisplayUnits, EdgeKind, Piece, PieceView } from '@drapewright/engine'
import { ChevronRight } from 'lucide-react'
import { type ReactNode, useEffect, useRef, useState } from 'react'
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
import type { PartRef, Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { CornerChooser } from './CornerChooser.tsx'
import { LengthField } from './LengthField.tsx'
import { AlterSection, alterSummary } from './AlterSection.tsx'
import {
  CutSection,
  GrainSection,
  ManyCutSection,
  NotchSection,
  grainSummary,
} from './PieceDetails.tsx'
import { edgeLabel, sideLabel } from './labels.ts'

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
  const units = session.engine.document().settings.displayUnits
  const [x0, y0, x1, y1] = view.outlineBounds
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
  return (
    <section className="panel-section inspector" aria-labelledby="edge-heading">
      <h2 id="edge-heading">{t.inspector.edge}</h2>
      {sewn.map(({ seam, i }) => (
        <Button key={seam.id} className="link-button" onPress={() => session.selectSeam(seam.id)}>
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

function CornerSection({
  session,
  piece,
  edgeId,
}: {
  session: Session
  piece: Piece
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
 * Details of the selected piece, edge or corner, editable in sewing terms;
 * or of several pieces, set together. Each new choice shows from the top.
 */
export function Inspector({ session }: { session: Session }) {
  const { selection, part } = useStore(session.ui)
  useEngineState(session)
  const top = useRef<HTMLDivElement>(null)
  const chosen = [...selection].sort().join(',')
  useEffect(() => {
    top.current?.closest('.side-details')?.scrollTo({ top: 0 })
  }, [chosen])

  // In the pieces' order, whichever was chosen first.
  const scene = session.scene().filter((sp) => selection.has(sp.piece.id))
  let body: ReactNode
  if (scene.length === 0) {
    body = (
      <section className="panel-section">
        <p className="muted small">{t.inspector.nothing}</p>
      </section>
    )
  } else if (scene.length > 1) {
    body = <ManyPiecesSection session={session} pieces={scene.map((sp) => sp.piece)} />
  } else {
    const { piece, view } = scene[0]!
    const own: PartRef | null = part?.piece === piece.id ? part : null
    body = (
      <>
        {own?.kind === 'corner' && (
          <CornerSection session={session} piece={piece} edgeId={own.edge} />
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
