import { engineVersion } from '@soom/engine'
import { Info } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button, Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from 'react-aria-components'
import { type Capabilities, detectCapabilities } from '../platform/capabilities.ts'
import { t } from '../strings/en.ts'

function Checks() {
  const [caps, setCaps] = useState<Capabilities | null>(null)
  useEffect(() => {
    void detectCapabilities().then(setCaps)
  }, [])
  if (!caps) return <p className="muted">{t.device.checking}</p>
  const rows: [readonly [string, string, string], boolean][] = [
    [t.device.storage, caps.opfs],
    [t.device.offline, caps.serviceWorker],
    [t.device.gpu, caps.webgpu],
    [t.device.isolation, caps.crossOriginIsolated],
  ]
  return (
    <ul className="checks">
      {rows.map(([[label, ok, fallback], good]) => (
        <li key={label} data-ok={good}>
          <span className="check-mark" aria-hidden="true">
            {good ? '✓' : '–'}
          </span>
          <span>
            <strong>{label}</strong>
            <span className="muted">{good ? ok : fallback}</span>
          </span>
        </li>
      ))}
    </ul>
  )
}

/** What this browser can do, behind the ⓘ button. */
export function DeviceDialog() {
  return (
    <DialogTrigger>
      <Button className="icon-button" aria-label={t.actions.device}>
        <Info size={20} aria-hidden="true" />
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{t.device.heading}</Heading>
                <Checks />
                <p className="muted small" data-testid="engine-status">
                  {t.device.engineVersion(engineVersion())}
                </p>
                <div className="dialog-actions">
                  <Button className="button" onPress={close}>
                    {t.actions.close}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}
