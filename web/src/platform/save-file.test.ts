import { describe, expect, it } from 'vitest'
import { fileName } from './save-file.ts'

describe('fileName', () => {
  it('keeps the project name, less what file systems reject', () => {
    expect(fileName('A-line skirt', 'pdf')).toBe('A-line skirt.pdf')
    expect(fileName('Front/back: v2?', 'soom')).toBe('Front-back- v2-.soom')
    expect(fileName('Rok “Anna” ✂︎', 'pdf')).toBe('Rok “Anna” ✂︎.pdf')
  })

  it('never makes an empty name', () => {
    expect(fileName('   ', 'pdf')).toBe('pattern.pdf')
  })
})
