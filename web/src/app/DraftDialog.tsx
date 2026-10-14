// A FreeSewing design drafted to someone's measurements, and opened as a
// pattern (its pieces then added like any pattern's). Which of their
// measurements it uses is said, and which FreeSewing estimates.

import { measurementVocabulary } from '@drapewright/engine'
import { ChevronDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import {
  Button,
  Dialog,
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
} from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'
import {
  DESIGNS,
  type DesignName,
  draftSvg,
  estimated,
  freesewingMeasurements,
  needs,
} from './freesewing.ts'
import { withExample } from './example.ts'

const text = t.drafting

/** A FreeSewing measurement's name: as Drapewright names it, or in words. */
function measurementName(name: string): string {
  const ours = measurementVocabulary().find((m) => m.freesewing === name)
  const said = ours
    ? (t.measure.names as Record<string, [string, string]>)[ours.key]?.[0]
    : undefined
  if (said) return said.toLowerCase()
  return name
    .replace(/([A-Z])/g, ' $1')
    .toLowerCase()
    .replace(/^hps\b/, 'HPS')
}

/** The choice of a design to draft, while the session asks for it. */
export function DraftDialog({ session }: { session: Session }) {
  const { drafting } = useStore(session.ui)
  return (
    <ModalOverlay
      isDismissable
      isOpen={drafting}
      onOpenChange={(open) => session.showDrafting(open)}
      className="modal-overlay"
    >
      <Modal className="modal wide">
        <Dialog className="dialog">
          {({ close }) => <DraftForm session={session} close={close} />}
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}

interface Plan {
  design: DesignName
  set: string
  used: string[]
  guessed: Record<string, number>
}

function DraftForm({ session, close }: { session: Session; close: () => void }) {
  useEngineState(session)
  // Each person measured, then the example: someone to draft to before
  // anyone is measured.
  const sets = withExample(session.engine.document().measurementSets)
  const [setId, setSetId] = useState(sets[0]!.id)
  const set = sets.find((s) => s.id === setId) ?? sets[0]!
  const [design, setDesign] = useState<DesignName>('teagan')
  const [plan, setPlan] = useState<Plan | null>(null)
  const [drafting, setDrafting] = useState(false)
  const [failed, setFailed] = useState<string | null>(null)

  // What the design needs of this person, and what is estimated.
  useEffect(() => {
    let current = true
    const have = freesewingMeasurements(set)
    void needs(design)
      .then(async ({ required, optional }) => {
        const guessed = await estimated(have, required, set.figure !== 'man')
        const used = [...required, ...optional].filter((m) => m in have)
        if (current) setPlan({ design, set: set.id, used, guessed })
      })
      .catch((error: unknown) => {
        if (current) setFailed(text.failed(error instanceof Error ? error.message : String(error)))
      })
    return () => {
      current = false
    }
  }, [design, set])

  const ready = plan?.design === design && plan.set === set.id ? plan : null
  const draft = async () => {
    if (!ready) return
    setDrafting(true)
    setFailed(null)
    try {
      const have = freesewingMeasurements(set)
      const measurements = Object.fromEntries(ready.used.map((m) => [m, have[m]!]))
      const svg = await draftSvg(design, { ...measurements, ...ready.guessed })
      const name = `${text.designs[design][0]} ${text.forWhom(set.name)}.svg`
      close()
      await session.openPatternFile(new File([svg], name, { type: 'image/svg+xml' }))
    } catch (error) {
      setFailed(text.failed(error instanceof Error ? error.message : String(error)))
    } finally {
      setDrafting(false)
    }
  }

  return (
    <>
      <Heading slot="title">{text.heading}</Heading>
      <p className="muted">{text.intro}</p>
      {sets.length > 1 && (
        <Select
          className="field compact"
          selectedKey={set.id}
          onSelectionChange={(id) => setSetId(String(id))}
        >
          <Label>{text.whose}</Label>
          <Button className="select-button">
            <SelectValue>{() => set.name}</SelectValue>
            <ChevronDown size={16} aria-hidden="true" />
          </Button>
          <Popover className="popover">
            <ListBox className="menu">
              {sets.map((s) => (
                <ListBoxItem key={s.id} id={s.id} className="menu-item">
                  {s.name}
                </ListBoxItem>
              ))}
            </ListBox>
          </Popover>
        </Select>
      )}
      <RadioGroup
        className="design-choice"
        value={design}
        onChange={(d) => setDesign(d as DesignName)}
      >
        <Label className="group-label">{text.which}</Label>
        <div className="design-cards">
          {DESIGNS.map((d) => (
            <Radio key={d} value={d} className="radio-card">
              <span className="radio-title">{text.designs[d][0]}</span>
              <span className="muted small">{text.designs[d][1]}</span>
            </Radio>
          ))}
        </div>
      </RadioGroup>
      {ready && (
        <p className="small" role="status">
          {text.uses(set.name, ready.used.map(measurementName))}{' '}
          {Object.keys(ready.guessed).length > 0 &&
            text.guessed(Object.keys(ready.guessed).map(measurementName))}
        </p>
      )}
      {failed && <p className="measure-warning small">{failed}</p>}
      <p className="muted small">
        {text.credit}{' '}
        <a href="/licences/freesewing.txt" target="_blank" rel="noopener">
          {t.samples.licence}
        </a>
      </p>
      <div className="dialog-actions">
        <Button className="button" onPress={close}>
          {text.cancel}
        </Button>
        <Button
          className="button primary"
          isDisabled={!ready || drafting}
          onPress={() => void draft()}
        >
          {drafting ? text.drafting : text.draft(text.designs[design][0])}
        </Button>
      </div>
    </>
  )
}
