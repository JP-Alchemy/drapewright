// Hands a file to the browser to save (or, on iPad, to open and share).
// Nothing leaves the device.

export function saveFile(bytes: Uint8Array, name: string, type: string): void {
  const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type }))
  const link = document.createElement('a')
  link.href = url
  link.download = name
  link.rel = 'noopener'
  document.body.append(link)
  link.click()
  link.remove()
  // Some browsers read the file after the click returns.
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}

/** A file name from a project name, without characters file systems reject. */
export function fileName(name: string, extension: string): string {
  const safe = [...name]
    .map((c) => (c < ' ' || '\\/:*?"<>|'.includes(c) ? '-' : c))
    .join('')
    .trim()
    .slice(0, 80)
  return `${safe || 'pattern'}.${extension}`
}
