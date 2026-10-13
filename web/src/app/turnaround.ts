// A video turning once round the figure, made from the view's pictures:
// each drawn on a canvas in turn, at a steady rate, recorded as the
// browser records video (WebM, or MP4 in Safari).

/** The kinds of video to try, best first. */
const KINDS = ['video/webm;codecs=vp9', 'video/webm', 'video/mp4']

/** Whether this browser can record video from a canvas. */
export function canRecord(): boolean {
  return (
    typeof MediaRecorder !== 'undefined' &&
    typeof HTMLCanvasElement.prototype.captureStream === 'function' &&
    KINDS.some((k) => MediaRecorder.isTypeSupported(k))
  )
}

/** `frames` played `fps` a second, as a video; and its file's extension. */
export async function video(frames: Blob[], fps = 24): Promise<{ blob: Blob; extension: string }> {
  const kind = KINDS.find((k) => MediaRecorder.isTypeSupported(k))
  if (!kind) throw new Error('This browser can’t record video.')
  const pictures = await Promise.all(frames.map((f) => createImageBitmap(f)))
  const first = pictures[0]
  if (!first) throw new Error('There was nothing to record.')
  const canvas = document.createElement('canvas')
  canvas.width = first.width
  canvas.height = first.height
  const context = canvas.getContext('2d')!
  context.drawImage(first, 0, 0)
  const stream = canvas.captureStream(0)
  const track = stream.getVideoTracks()[0] as CanvasCaptureMediaStreamTrack
  const recorder = new MediaRecorder(stream, { mimeType: kind, videoBitsPerSecond: 6_000_000 })
  const parts: Blob[] = []
  recorder.ondataavailable = (e) => e.data.size > 0 && parts.push(e.data)
  const stopped = new Promise<void>((resolve) => (recorder.onstop = () => resolve()))
  recorder.start()
  for (const picture of pictures) {
    context.drawImage(picture, 0, 0)
    track.requestFrame()
    await new Promise((resolve) => setTimeout(resolve, 1000 / fps))
  }
  recorder.stop()
  await stopped
  for (const picture of pictures) picture.close()
  track.stop()
  return {
    blob: new Blob(parts, { type: kind.split(';')[0] }),
    extension: kind.startsWith('video/mp4') ? 'mp4' : 'webm',
  }
}
