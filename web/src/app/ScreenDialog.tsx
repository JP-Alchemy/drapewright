import { Minus, Plus, Ruler } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Dialog,
  DialogTrigger,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Slider,
  SliderThumb,
  SliderTrack,
} from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'

/** ISO/IEC 7810 ID-1: bank cards, driving licences and most ID cards, mm. */
const CARD = { width: 85.6, height: 53.98, radius: 3.18 }
/** The ruler line, mm. */
const LINE = 50
/** CSS px per mm the slider covers: big desktop monitors to dense phones. */
const MIN = 2
const MAX = 8
/** One tap of − or +. */
const NUDGE = 1.0025

const clamp = (v: number) => Math.min(MAX, Math.max(MIN, v))

/** The ruler button in the zoom controls, and the dialog that measures the screen. */
export function ScreenDialog({ session }: { session: Session }) {
  const { screenMeasured } = useStore(session.ui)
  return (
    <DialogTrigger>
      <Button
        className="icon-button"
        aria-label={screenMeasured ? t.screen.open : t.screen.openUnmeasured}
        data-unmeasured={screenMeasured ? undefined : true}
      >
        <Ruler size={18} aria-hidden="true" />
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => <ScreenForm session={session} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}

function ScreenForm({ session, close }: { session: Session; close: () => void }) {
  const [scale, setScale] = useState(() => session.ui.get().viewport.pxPerMm)
  const finish = (pxPerMm: number | null, message: string) => {
    session.setScreenScale(pxPerMm)
    session.toast(message)
    close()
  }
  return (
    <>
      <Heading slot="title">{t.screen.heading}</Heading>
      <p>{t.screen.intro}</p>
      <div className="card-stage">
        <div
          className="card-outline"
          role="img"
          aria-label={t.screen.card}
          data-testid="card-outline"
          style={{
            width: CARD.width * scale,
            height: CARD.height * scale,
            borderRadius: CARD.radius * scale,
          }}
        />
      </div>
      <div className="slider-row">
        <Button
          className="icon-button"
          aria-label={t.screen.smaller}
          onPress={() => setScale((s) => clamp(s / NUDGE))}
        >
          <Minus size={18} aria-hidden="true" />
        </Button>
        <Slider
          className="slider"
          value={scale}
          minValue={MIN}
          maxValue={MAX}
          step={0.01}
          onChange={setScale}
        >
          <Label className="visually-hidden">{t.screen.size}</Label>
          <SliderTrack className="slider-track">
            <SliderThumb className="slider-thumb" />
          </SliderTrack>
        </Slider>
        <Button
          className="icon-button"
          aria-label={t.screen.larger}
          onPress={() => setScale((s) => clamp(s * NUDGE))}
        >
          <Plus size={18} aria-hidden="true" />
        </Button>
      </div>
      <p className="muted small">{t.screen.ruler}</p>
      <div
        className="ruler-line"
        role="img"
        aria-label={t.screen.rulerLabel}
        data-testid="ruler-line"
        style={{ width: LINE * scale }}
      />
      <div className="dialog-actions">
        <Button className="button" onPress={() => finish(null, t.screen.standard)}>
          {t.screen.reset}
        </Button>
        <Button className="button" onPress={close}>
          {t.screen.cancel}
        </Button>
        <Button className="button primary" onPress={() => finish(scale, t.screen.saved)}>
          {t.screen.save}
        </Button>
      </div>
    </>
  )
}
