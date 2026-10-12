// A fabric of one's own, measured at home: a square weighed, strips
// stretched under a weight and slid out over a table's edge. What is not
// measured is taken from the ready-made fabric it is most like.

import type { FabricPreset, Measured, OwnFabric } from '@drapewright/engine'
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Dialog,
  Group,
  Heading,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Modal,
  ModalOverlay,
  NumberField,
  Popover,
  Select,
  SelectValue,
  Text,
  TextField,
} from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'

/** The ready-made fabrics, in the order offered. */
const PRESETS = Object.keys(t.avatar.fabrics) as FabricPreset[]

/** One measurement, in `unit`; left empty if not measured. */
function Measurement({
  label,
  howTo,
  unit,
  value,
  onChange,
  max,
}: {
  label: string
  howTo: string
  unit: string
  value: number | null
  onChange(v: number | null): void
  max: number
}) {
  return (
    <NumberField
      className="field"
      value={value ?? Number.NaN}
      minValue={0}
      maxValue={max}
      formatOptions={{ maximumFractionDigits: 1 }}
      onChange={(v) => onChange(Number.isFinite(v) ? v : null)}
    >
      <Label>{label}</Label>
      <Group className="number-group with-unit">
        <Input className="number-input" />
        <span className="number-unit">{unit}</span>
      </Group>
      <Text slot="description" className="muted small">
        {howTo}
      </Text>
    </NumberField>
  )
}

/**
 * Measuring a fabric of one's own (`fabric` null), or changing one. Saved,
 * `onSaved` hears its id.
 */
export function FabricDialog({
  session,
  fabric,
  isOpen,
  onClose,
  onSaved,
}: {
  session: Session
  fabric: OwnFabric | null
  isOpen: boolean
  onClose(): void
  onSaved(id: string): void
}) {
  return (
    <ModalOverlay
      isDismissable
      isOpen={isOpen}
      onOpenChange={(open) => !open && onClose()}
      className="modal-overlay"
    >
      <Modal className="modal">
        <Dialog className="dialog">
          {({ close }) => (
            <FabricForm session={session} fabric={fabric} close={close} onSaved={onSaved} />
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}

function FabricForm({
  session,
  fabric,
  close,
  onSaved,
}: {
  session: Session
  fabric: OwnFabric | null
  close(): void
  onSaved(id: string): void
}) {
  const text = t.fabric
  const was: Measured = fabric?.measured ?? {}
  const [name, setName] = useState(fabric?.name ?? '')
  const [like, setLike] = useState<FabricPreset>(fabric?.like ?? 'poplin')
  // As measured: the square's grams, how long the 10 cm grew to, and how
  // far the strip slid out.
  const [grams, setGrams] = useState(was.weight === undefined ? null : was.weight / 100)
  const grown = (stretch: number | undefined) => (stretch === undefined ? null : 10 + stretch / 10)
  const [along, setAlong] = useState(grown(was.lengthwise))
  const [across, setAcross] = useState(grown(was.crosswise))
  const [overhang, setOverhang] = useState(was.overhang ?? null)
  const save = () => {
    const measured: Measured = {}
    if (grams !== null) measured.weight = grams * 100
    if (along !== null) measured.lengthwise = Math.max(0, (along - 10) * 10)
    if (across !== null) measured.crosswise = Math.max(0, (across - 10) * 10)
    if (overhang !== null) measured.overhang = overhang
    const next: OwnFabric = { id: fabric?.id ?? '', name: name.trim(), like, measured }
    const saved = session.dispatch(
      fabric ? { type: 'setFabric', fabric: next } : { type: 'addFabric', fabric: next },
    )
    if (!saved) return
    const id = fabric?.id ?? session.engine.document().fabrics.at(-1)?.id
    if (id) onSaved(id)
    close()
  }
  const remove = () => {
    if (fabric && session.dispatch({ type: 'deleteFabric', id: fabric.id })) close()
  }
  return (
    <>
      <Heading slot="title">{fabric ? text.change(fabric.name) : text.heading}</Heading>
      <p className="muted small">{text.intro}</p>
      <TextField className="field" value={name} onChange={setName} autoFocus>
        <Label>{text.name}</Label>
        <Input className="text-input" placeholder={text.namePlaceholder} />
      </TextField>
      <Select
        className="field"
        selectedKey={like}
        onSelectionChange={(key) => setLike(key as FabricPreset)}
      >
        <Label>{text.like}</Label>
        <Button className="select-button">
          <SelectValue />
          <ChevronDown size={16} aria-hidden="true" />
        </Button>
        <Text slot="description" className="muted small">
          {text.likeHint}
        </Text>
        <Popover className="popover">
          <ListBox className="menu">
            {PRESETS.map((p) => (
              <ListBoxItem key={p} id={p} className="menu-item">
                {t.avatar.fabrics[p]}
              </ListBoxItem>
            ))}
          </ListBox>
        </Popover>
      </Select>
      <Measurement
        label={text.weight}
        howTo={text.weightHow}
        unit="g"
        value={grams}
        onChange={setGrams}
        max={20}
      />
      <Measurement
        label={text.along}
        howTo={text.stretchHow(true)}
        unit="cm"
        value={along}
        onChange={setAlong}
        max={40}
      />
      <Measurement
        label={text.across}
        howTo={text.stretchHow(false)}
        unit="cm"
        value={across}
        onChange={setAcross}
        max={40}
      />
      <Measurement
        label={text.overhang}
        howTo={text.overhangHow}
        unit="cm"
        value={overhang}
        onChange={setOverhang}
        max={60}
      />
      <div className="dialog-actions">
        {fabric && (
          <Button className="button danger" onPress={remove}>
            {text.remove}
          </Button>
        )}
        <div className="spacer" />
        <Button className="button" onPress={close}>
          {text.cancel}
        </Button>
        <Button className="button primary" isDisabled={!name.trim()} onPress={save}>
          {text.save}
        </Button>
      </div>
    </>
  )
}
