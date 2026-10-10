// Projects on this device, kept by the IO worker. Each project is a few
// files:
//
//   <id>/project.json           the project, as saved
//   <id>/next.json              written first; a complete copy if a save is
//                               cut short
//   <id>/sources/<sha>-<name>   files the pieces were imported from
//
// They live in the origin private file system (OPFS) where the browser
// allows it, otherwise in IndexedDB (private browsing in Safari, for one).
// IndexedDB also keeps a short summary of each project, so the projects
// list opens without reading every project. Nothing here leaves the device.

import type { SourceFile } from '@soom/engine'
import { type IDBPDatabase, openDB } from 'idb'

export interface ProjectSummary {
  id: string
  name: string
  /** RFC 3339. */
  modified: string
  pieces: number
}

/** Where a project's files are kept. Paths are relative to the project. */
interface Files {
  readonly kind: 'opfs' | 'indexeddb'
  write(id: string, path: string, data: Uint8Array): Promise<void>
  read(id: string, path: string): Promise<Uint8Array | null>
  /** Names of the files in one of a project's folders. */
  list(id: string, folder: string): Promise<string[]>
  removeProject(id: string): Promise<void>
}

const SUMMARIES = 'projects'
const FILES = 'files'
let db: Promise<IDBPDatabase> | undefined

function database(): Promise<IDBPDatabase> {
  db ??= openDB('soom', 2, {
    upgrade(d, oldVersion) {
      if (oldVersion < 1) d.createObjectStore(SUMMARIES, { keyPath: 'id' })
      if (oldVersion < 2) d.createObjectStore(FILES)
    },
  })
  return db
}

// Worker-only OPFS calls that TypeScript's DOM types leave out. Older
// Safari returns promises from these, so every call is awaited.
interface SyncAccessHandle {
  truncate(size: number): unknown
  write(data: Uint8Array, options?: { at?: number }): unknown
  flush(): unknown
  close(): unknown
}
interface WorkerFileHandle extends FileSystemFileHandle {
  createSyncAccessHandle(): Promise<SyncAccessHandle>
}
interface IterableDirectory extends FileSystemDirectoryHandle {
  entries(): AsyncIterableIterator<[string, FileSystemFileHandle | FileSystemDirectoryHandle]>
}

const notFound = (error: unknown) => error instanceof DOMException && error.name === 'NotFoundError'

async function writeHandle(dir: FileSystemDirectoryHandle, name: string, data: Uint8Array) {
  const file = (await dir.getFileHandle(name, { create: true })) as WorkerFileHandle
  const access = await file.createSyncAccessHandle()
  try {
    await access.truncate(0)
    await access.write(data, { at: 0 })
    await access.flush()
  } finally {
    await access.close()
  }
}

function opfs(projects: FileSystemDirectoryHandle): Files {
  /** The folder holding `path`, and the file's name in it. */
  const locate = async (id: string, path: string, create: boolean) => {
    const parts = path.split('/')
    let dir = await projects.getDirectoryHandle(id, { create })
    for (const folder of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(folder, { create })
    return { dir, name: parts.at(-1) ?? path }
  }
  return {
    kind: 'opfs',
    async write(id, path, data) {
      const { dir, name } = await locate(id, path, true)
      await writeHandle(dir, name, data)
    },
    async read(id, path) {
      try {
        const { dir, name } = await locate(id, path, false)
        const file = await (await dir.getFileHandle(name)).getFile()
        return new Uint8Array(await file.arrayBuffer())
      } catch (error) {
        if (notFound(error)) return null
        throw error
      }
    },
    async list(id, folder) {
      try {
        const project = await projects.getDirectoryHandle(id)
        const dir = (await project.getDirectoryHandle(folder)) as IterableDirectory
        const names: string[] = []
        for await (const [name, handle] of dir.entries())
          if (handle.kind === 'file') names.push(name)
        return names
      } catch (error) {
        if (notFound(error)) return []
        throw error
      }
    },
    async removeProject(id) {
      try {
        await projects.removeEntry(id, { recursive: true })
      } catch (error) {
        if (!notFound(error)) throw error
      }
    },
  }
}

function indexedDb(): Files {
  const range = (id: string, prefix: string) => IDBKeyRange.bound([id, prefix], [id, `${prefix}￿`])
  return {
    kind: 'indexeddb',
    async write(id, path, data) {
      await (await database()).put(FILES, data, [id, path])
    },
    async read(id, path) {
      return ((await (await database()).get(FILES, [id, path])) as Uint8Array | undefined) ?? null
    },
    async list(id, folder) {
      const keys = (await (await database()).getAllKeys(FILES, range(id, `${folder}/`))) as [
        string,
        string,
      ][]
      return keys.map(([, path]) => path.slice(folder.length + 1))
    },
    async removeProject(id) {
      await (await database()).delete(FILES, range(id, ''))
    },
  }
}

/** OPFS if it can really be written here, otherwise IndexedDB. */
async function chooseFiles(): Promise<Files> {
  try {
    const root = await navigator.storage.getDirectory()
    const projects = await root.getDirectoryHandle('projects', { create: true })
    await writeHandle(root, '.probe', new Uint8Array([1]))
    await root.removeEntry('.probe')
    return opfs(projects)
  } catch {
    return indexedDb()
  }
}

let chosen: Promise<Files> | undefined
const files = () => (chosen ??= chooseFiles())

const parses = (text: string) => {
  try {
    JSON.parse(text)
    return true
  } catch {
    return false
  }
}

// Writes run one at a time, in order, so two never touch the same files.
let queue: Promise<unknown> = Promise.resolve()
function serially<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task)
  queue = run.catch(() => undefined)
  return run
}

export const storageJobs = {
  /** Saves a project: `next.json` first, then `project.json`. */
  saveProject(summary: ProjectSummary, json: string): Promise<void> {
    return serially(async () => {
      const store = await files()
      const bytes = new TextEncoder().encode(json)
      await store.write(summary.id, 'next.json', bytes)
      await store.write(summary.id, 'project.json', bytes)
      await (await database()).put(SUMMARIES, summary)
    })
  },

  /** The saved project text; falls back to `next.json` if a save was cut short. */
  async loadProject(id: string): Promise<string> {
    const store = await files()
    for (const name of ['project.json', 'next.json']) {
      const bytes = await store.read(id, name)
      const text = bytes && new TextDecoder().decode(bytes)
      if (text && parses(text)) return text
    }
    throw new Error('This project could not be read from this device’s storage.')
  },

  /** Every saved project, most recently changed first. */
  async listProjects(): Promise<ProjectSummary[]> {
    const all = (await (await database()).getAll(SUMMARIES)) as ProjectSummary[]
    return all.sort((a, b) => b.modified.localeCompare(a.modified))
  },

  /** Deletes a project and its files from this device. */
  deleteProject(id: string): Promise<void> {
    return serially(async () => {
      await (await files()).removeProject(id)
      await (await database()).delete(SUMMARIES, id)
    })
  },

  /** Keeps a file the project was imported from. */
  saveSource(id: string, source: SourceFile): Promise<void> {
    return serially(async () => {
      const name = source.path.replace(/^sources\//, '')
      await (await files()).write(id, `sources/${name}`, source.data)
    })
  },

  /** The project's kept source files whose names start with one of `prefixes`. */
  async loadSources(id: string, prefixes: string[]): Promise<SourceFile[]> {
    const store = await files()
    const out: SourceFile[] = []
    for (const name of await store.list(id, 'sources')) {
      if (!prefixes.some((p) => name.startsWith(p))) continue
      const data = await store.read(id, `sources/${name}`)
      if (data) out.push({ path: `sources/${name}`, data })
    }
    return out.sort((a, b) => a.path.localeCompare(b.path))
  },

  /** Where projects are kept, and whether the browser promised not to clear them. */
  async storageStatus(): Promise<{ kind: 'opfs' | 'indexeddb'; persisted: boolean }> {
    const persisted = (await navigator.storage?.persisted?.().catch(() => false)) ?? false
    return { kind: (await files()).kind, persisted }
  },
}

export type StorageJobs = typeof storageJobs
