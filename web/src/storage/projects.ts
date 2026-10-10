// Projects on this device, from the main thread: the open project is saved
// a moment after each change; others are listed, opened and deleted; any
// project can be backed up as a .soom file and restored from one. The IO
// worker does the reading and writing (see worker-store.ts).

import { Engine, type Issue } from '@soom/engine'
import { type Store, createStore } from '../state/store.ts'
import { t } from '../strings/en.ts'
import type { IoClient } from '../workers/io-client.ts'
import type { ProjectSummary } from './worker-store.ts'

export type { ProjectSummary }

/** Saves this long after the last change, ms. */
const AUTOSAVE_DELAY = 800

export type SaveStatus =
  | { state: 'idle' }
  | { state: 'saving' }
  | { state: 'saved' }
  | { state: 'failed'; message: string }

export interface Opened {
  engine: Engine
  issues: Issue[]
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

function summaryOf(engine: Engine): ProjectSummary {
  const doc = engine.document()
  return {
    id: doc.meta.id,
    name: doc.meta.name,
    modified: new Date().toISOString(),
    pieces: doc.pieces.length,
  }
}

/** A name that is safe as one file under `sources/`. */
export function sourceName(sha256: string, fileName: string): string {
  const clean = [...fileName]
    .map((c) => (c < ' ' || '/\\'.includes(c) ? '-' : c))
    .join('')
    .replace(/^\.+/, '')
    .slice(0, 120)
  return `sources/${sha256}-${clean || 'file'}`
}

export class Projects {
  readonly status: Store<SaveStatus> = createStore<SaveStatus>({ state: 'idle' })
  #saving: Promise<void> = Promise.resolve()
  #persistAsked = false

  constructor(readonly io: IoClient) {}

  list(): Promise<ProjectSummary[]> {
    return this.io.call('listProjects')
  }

  create(): Engine {
    return Engine.create(t.untitled)
  }

  async open(id: string): Promise<Opened> {
    return Engine.open(await this.io.call('loadProject', id))
  }

  /** The most recently changed project, or a new one if there is none. */
  async openLatest(): Promise<Opened & { failed?: string }> {
    let latest: ProjectSummary | undefined
    try {
      latest = (await this.list())[0]
    } catch {
      // No storage in this browser: start a new project.
    }
    if (!latest) return { engine: this.create(), issues: [] }
    try {
      const opened = await this.open(latest.id)
      this.status.set({ state: 'saved' })
      return opened
    } catch (error) {
      return { engine: this.create(), issues: [], failed: message(error) }
    }
  }

  /**
   * Saves the project a moment after each change, and at once when the
   * page is hidden; `beforeSave` runs first (to note the view). Returns a
   * function that stops it, saving what is left.
   */
  autosave(engine: Engine, beforeSave: () => void = () => {}): () => void {
    let timer: ReturnType<typeof setTimeout> | undefined
    let dirty = false
    const save = () => {
      clearTimeout(timer)
      if (!dirty) return
      dirty = false
      beforeSave()
      const summary = summaryOf(engine)
      const json = engine.toJson()
      this.#saving = this.#saving
        .then(() => this.io.call('saveProject', summary, json))
        .then(
          () => {
            if (!dirty) this.status.set({ state: 'saved' })
            this.#askToPersist()
          },
          (error: unknown) => {
            dirty = true
            this.status.set({ state: 'failed', message: message(error) })
          },
        )
    }
    const unsubscribe = engine.subscribe(() => {
      dirty = true
      this.status.set({ state: 'saving' })
      clearTimeout(timer)
      timer = setTimeout(save, AUTOSAVE_DELAY)
    })
    const onHidden = () => {
      if (document.visibilityState === 'hidden') save()
    }
    document.addEventListener('visibilitychange', onHidden)
    window.addEventListener('pagehide', save)
    return () => {
      save()
      unsubscribe()
      document.removeEventListener('visibilitychange', onHidden)
      window.removeEventListener('pagehide', save)
    }
  }

  /** Waits for saves already started. */
  flush(): Promise<void> {
    return this.#saving
  }

  /** Keeps a file the project's pieces were imported from. Best effort. */
  keepSource(projectId: string, sha256: string, fileName: string, data: Uint8Array): void {
    void this.io
      .call('saveSource', projectId, { path: sourceName(sha256, fileName), data })
      .catch(() => {
        // Without storage the project is still usable; a backup then
        // simply has no copy of the original file.
      })
  }

  /** The project as a .soom file, with the source files its pieces came from. */
  async backup(engine: Engine): Promise<Uint8Array> {
    const doc = engine.document()
    const hashes = new Set(doc.pieces.flatMap((p) => (p.source?.sha256 ? [p.source.sha256] : [])))
    const sources = await this.io
      .call(
        'loadSources',
        doc.meta.id,
        [...hashes].map((h) => `${h}-`),
      )
      .catch(() => [])
    return this.io.call('packSoom', engine.toJson(), sources)
  }

  /**
   * Opens a .soom file as a project on this device. A backup of a project
   * that is already here becomes a copy beside it; nothing is overwritten.
   */
  async restore(bytes: Uint8Array): Promise<Opened> {
    const { projectJson, sources } = await this.io.call('unpackSoom', bytes)
    let json = projectJson
    const parsed = JSON.parse(projectJson) as { meta?: { id?: string; name?: string } }
    const existing = await this.list().catch(() => [])
    if (parsed.meta && existing.some((p) => p.id === parsed.meta?.id)) {
      parsed.meta.id = crypto.randomUUID()
      parsed.meta.name = t.projects.restoredName(parsed.meta.name ?? t.untitled)
      json = JSON.stringify(parsed)
    }
    const opened = Engine.open(json)
    const id = opened.engine.document().meta.id
    for (const source of sources) {
      await this.io.call('saveSource', id, source).catch(() => undefined)
    }
    await this.io.call('saveProject', summaryOf(opened.engine), opened.engine.toJson())
    return opened
  }

  delete(id: string): Promise<void> {
    return this.io.call('deleteProject', id)
  }

  /** Asks once for storage the browser will not clear on its own. */
  #askToPersist() {
    if (this.#persistAsked) return
    this.#persistAsked = true
    void navigator.storage?.persist?.().catch(() => false)
  }
}
