// Patterns to try Drapewright with, before opening one's own: a blouse and
// a skirt drawn for its tests (CC0), sewn and ready to drape, and
// FreeSewing's Sasha dress as a copy-shop PDF (MIT), to see a PDF put
// together.

import blouse from '../../../fixtures/drapewright/blouse.drapewright?url'
import skirt from '../../../fixtures/drapewright/skirt.drapewright?url'
import dress from '../../../fixtures/pdf/freesewing-sasha-a0.pdf?url'
import { t } from '../strings/en.ts'

export type SampleName = 'blouse' | 'skirt' | 'dress'

export interface Sample {
  url: string
  /** The file's name, as if opened from the device. */
  file: string
  /** A project, sewn already, or a pattern to import. */
  kind: 'project' | 'pattern'
  /** Its licence, where it isn't Drapewright's own (CC0). */
  licence?: string
}

export const SAMPLES: Record<SampleName, Sample> = {
  blouse: { url: blouse, file: 'sample-blouse.drapewright', kind: 'project' },
  skirt: { url: skirt, file: 'sample-skirt.drapewright', kind: 'project' },
  dress: {
    url: dress,
    file: 'freesewing-sasha-dress-a0.pdf',
    kind: 'pattern',
    // A copy of fixtures/svg/LICENSE-freesewing, served as text.
    licence: '/licences/freesewing.txt',
  },
}

export const isSampleName = (name: string | null): name is SampleName =>
  name !== null && Object.hasOwn(SAMPLES, name)

/** The sample's file, fetched. */
export async function sampleFile(name: SampleName): Promise<File> {
  const sample = SAMPLES[name]
  const response = await fetch(sample.url)
  if (!response.ok) throw new Error(t.samples.unavailable)
  return new File([await response.blob()], sample.file)
}
