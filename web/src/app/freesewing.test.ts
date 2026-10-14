import { cisFemaleAdult36 } from '@freesewing/models'
import { describe, expect, it } from 'vitest'
import { draftSvg } from './freesewing.ts'

describe('draftSvg', () => {
  it('drafts Sasha without its pockets, which cannot be sewn yet', async () => {
    const svg = await draftSvg('sasha', cisFemaleAdult36 as Record<string, number>)
    const parts = [...svg.matchAll(/id="fs-stack-[^"]*-part-([^"]+)"/g)].map((m) => m[1])
    expect(parts).toEqual([
      'sasha.frontInside',
      'sasha.frontOutside',
      'sasha.backInside',
      'sasha.backOutside',
      'library.sleeve',
    ])
  })
})
