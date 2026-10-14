// A paper pattern piece from a photo: the photo shown, the four corners of
// a sheet of paper beside the piece tapped (they say how big things are,
// and how the table lies), then the piece tapped. Its outline is found and
// shown over it; used, it goes on to be imported as any pattern file does.

import type { PhotoRequest, Traced } from '@drapewright/engine'
import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Dialog,
  Heading,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Modal,
  ModalOverlay,
  Popover,
  Select,
  SelectValue,
  Slider,
  SliderOutput,
  SliderThumb,
  SliderTrack,
  TextField,
} from 'react-aria-components'
import { ChevronDown } from 'lucide-react'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'

type Pt = [number, number]

/** Sheets of paper to measure by, mm. */
const SHEETS = { a4: [210, 297], letter: [215.9, 279.4] } as const
type Sheet = keyof typeof SHEETS

/** Photos are read at most this many pixels on their longer side. */
const LONGEST = 2400
/** A tap this near a corner (CSS px) moves it. */
const NEAR = 24

/** The photo, read: its pixels at most `LONGEST` across, and as a picture to show. */
interface Read {
  rgba: Uint8Array
  width: number
  height: number
  bytes: Uint8Array
  url: string
}

async function read(file: File): Promise<Read> {
  const bitmap = await createImageBitmap(file)
  const k = Math.min(1, LONGEST / Math.max(bitmap.width, bitmap.height))
  const width = Math.max(1, Math.round(bitmap.width * k))
  const height = Math.max(1, Math.round(bitmap.height * k))
  const canvas = new OffscreenCanvas(width, height)
  const context = canvas.getContext('2d')!
  context.drawImage(bitmap, 0, 0, width, height)
  bitmap.close()
  const rgba = new Uint8Array(context.getImageData(0, 0, width, height).data.buffer)
  const bytes = new Uint8Array(await file.arrayBuffer())
  const url = URL.createObjectURL(await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.9 }))
  return { rgba, width, height, bytes, url }
}

export function PhotoDialog({ session }: { session: Session }) {
  const { photo } = useStore(session.ui)
  return (
    <ModalOverlay
      isDismissable
      isOpen={photo !== null}
      onOpenChange={(open) => !open && session.closePhoto()}
      className="modal-overlay"
    >
      <Modal className="modal wide">
        <Dialog className="dialog">
          {photo && (
            <PhotoForm key={photo.name + photo.lastModified} session={session} file={photo} />
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  )
}

function PhotoForm({ session, file }: { session: Session; file: File }) {
  const text = t.photo
  const [image, setImage] = useState<Read | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const [sheet, setSheet] = useState<Sheet>('a4')
  const [corners, setCorners] = useState<Pt[]>([])
  const [on, setOn] = useState<Pt | null>(null)
  const [tolerance, setTolerance] = useState(0.25)
  const [name, setName] = useState('')
  const [traced, setTraced] = useState<Traced | null>(null)
  const [why, setWhy] = useState<string | null>(null)
  const [tracing, setTracing] = useState(false)
  const shown = useRef<SVGSVGElement>(null)
  const dragging = useRef<number | null>(null)

  useEffect(() => {
    let live = true
    let url: string | null = null
    read(file).then(
      (r) => {
        url = r.url
        if (live) setImage(r)
        else URL.revokeObjectURL(r.url)
      },
      (error: unknown) => live && setFailed(error instanceof Error ? error.message : String(error)),
    )
    return () => {
      live = false
      if (url) URL.revokeObjectURL(url)
    }
  }, [file])

  // Traced again whenever what it needs changes.
  useEffect(() => {
    if (!image || corners.length < 4 || !on) return
    let live = true
    const request: PhotoRequest = {
      corners: corners as [Pt, Pt, Pt, Pt],
      sheet: [...SHEETS[sheet]],
      on,
      tolerance,
      name: text.untitled,
      file: file.name,
    }
    const timer = setTimeout(() => {
      setTracing(true)
      session
        .tracePhoto(image.rgba, image.width, image.height, image.bytes, request)
        .then(
          (found) => {
            if (!live) return
            setTraced(found)
            setWhy(null)
          },
          (error: unknown) => {
            if (!live) return
            setTraced(null)
            setWhy(error instanceof Error ? error.message : String(error))
          },
        )
        .finally(() => live && setTracing(false))
    }, 150)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [image, corners, on, sheet, tolerance, file, session, text.untitled])

  /** Where a pointer is on the photo, px. */
  const onPhoto = (event: { clientX: number; clientY: number }): Pt | null => {
    const svg = shown.current
    if (!svg || !image) return null
    const box = svg.getBoundingClientRect()
    return [
      ((event.clientX - box.left) / box.width) * image.width,
      ((event.clientY - box.top) / box.height) * image.height,
    ]
  }
  /** How many photo pixels a CSS pixel is. */
  const perCss = () => {
    const svg = shown.current
    return svg && image ? image.width / svg.getBoundingClientRect().width : 1
  }
  const down = (event: React.PointerEvent<SVGSVGElement>) => {
    const p = onPhoto(event)
    if (!p) return
    const near = corners.findIndex((c) => Math.hypot(c[0] - p[0], c[1] - p[1]) < NEAR * perCss())
    if (near >= 0) {
      dragging.current = near
      event.currentTarget.setPointerCapture(event.pointerId)
      return
    }
    if (corners.length < 4) setCorners([...corners, p])
    else setOn(p)
  }
  const move = (event: React.PointerEvent<SVGSVGElement>) => {
    const k = dragging.current
    const p = onPhoto(event)
    if (k === null || !p) return
    setCorners(corners.map((c, i) => (i === k ? p : c)))
  }
  const up = () => {
    dragging.current = null
  }

  const step = corners.length < 4 ? text.tapCorners(4 - corners.length) : on ? null : text.tapPiece
  const dot = image ? Math.max(image.width, image.height) / 120 : 8
  const said = tracing
    ? text.finding
    : why
      ? why
      : traced
        ? traced.cutOff
          ? text.cutOff
          : text.found
        : step

  return (
    <>
      <Heading slot="title">{text.heading}</Heading>
      <p className="muted small">{text.intro}</p>
      {failed && <p className="measure-warning">{text.unreadable(failed)}</p>}
      {image && (
        // Shaped as the photo is, so a tap on it is where it looks.
        <div
          className="photo-stage"
          style={{
            aspectRatio: `${image.width} / ${image.height}`,
            width: `min(100%, calc(60vh * ${(image.width / image.height).toFixed(4)}))`,
          }}
        >
          <img src={image.url} alt={text.photoLabel(file.name)} draggable={false} />
          <svg
            ref={shown}
            viewBox={`0 0 ${image.width} ${image.height}`}
            className="photo-marks"
            role="application"
            aria-label={step ?? text.found}
            onPointerDown={down}
            onPointerMove={move}
            onPointerUp={up}
            onPointerCancel={up}
          >
            {corners.length === 4 && (
              <polygon className="photo-sheet" points={corners.map((c) => c.join(',')).join(' ')} />
            )}
            {traced && (
              <polygon
                className="photo-outline"
                points={traced.outline.map((c) => c.join(',')).join(' ')}
              />
            )}
            {corners.map((c, i) => (
              <circle key={i} className="photo-corner" cx={c[0]} cy={c[1]} r={dot} />
            ))}
            {on && <circle className="photo-on" cx={on[0]} cy={on[1]} r={dot * 0.7} />}
          </svg>
        </div>
      )}
      <p className={`small${why || traced?.cutOff ? ' measure-warning' : ' muted'}`} role="status">
        {said}
      </p>
      <div className="photo-controls">
        <Select
          className="field"
          selectedKey={sheet}
          onSelectionChange={(s) => setSheet(s as Sheet)}
        >
          <Label>{text.sheet}</Label>
          <Button className="select-button">
            <SelectValue>{() => text.sheets[sheet]}</SelectValue>
            <ChevronDown size={16} aria-hidden="true" />
          </Button>
          <Popover className="popover">
            <ListBox className="menu">
              {(Object.keys(SHEETS) as Sheet[]).map((s) => (
                <ListBoxItem key={s} id={s} className="menu-item">
                  {text.sheets[s]}
                </ListBoxItem>
              ))}
            </ListBox>
          </Popover>
        </Select>
        <Slider
          className="field photo-tolerance"
          minValue={0.08}
          maxValue={0.6}
          step={0.01}
          value={tolerance}
          onChange={setTolerance}
        >
          <Label>{text.tolerance}</Label>
          <SliderOutput className="muted small">{() => text.toleranceSays(tolerance)}</SliderOutput>
          <SliderTrack className="slider-track">
            <SliderThumb className="slider-thumb" />
          </SliderTrack>
        </Slider>
        <TextField className="field" value={name} onChange={setName}>
          <Label>{text.name}</Label>
          <Input className="text-input" placeholder={text.untitled} />
        </TextField>
      </div>
      <div className="dialog-actions">
        <Button
          className="button"
          isDisabled={corners.length === 0}
          onPress={() => {
            setCorners([])
            setOn(null)
            setTraced(null)
            setWhy(null)
          }}
        >
          {text.again}
        </Button>
        <Button className="button" onPress={() => session.closePhoto()}>
          {text.cancel}
        </Button>
        <Button
          className="button primary"
          isDisabled={!traced || tracing || !image}
          onPress={() =>
            traced && image && session.usePhoto(traced, name.trim(), file.name, image.bytes)
          }
        >
          {text.use}
        </Button>
      </div>
    </>
  )
}
