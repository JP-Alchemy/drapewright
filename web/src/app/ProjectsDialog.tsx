import { ChevronDown, FilePlus2, Trash2, Upload } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button, Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { PROJECT_FILE_TYPES, type ProjectSummary } from '../storage/projects.ts'
import { t } from '../strings/en.ts'
import { CommitTextField } from './Inspector.tsx'
import type { SampleName } from './samples.ts'

export interface ProjectActions {
  /** Opens a saved project in place of the current one. */
  open(id: string): Promise<void>
  /** Starts a new, empty project. */
  create(): void
  /** Opens a .drapewright backup as a project. */
  restore(file: File): Promise<void>
  /** Opens one of the sample patterns: a project of its own, or imported. */
  openSample(name: SampleName): Promise<void>
  /** Deletes a saved project from this device. */
  remove(project: ProjectSummary): Promise<void>
  list(): Promise<ProjectSummary[]>
}

/** The project's name in the top bar; behind it, every project on this device. */
export function ProjectsDialog({
  session,
  actions,
}: {
  session: Session
  actions: ProjectActions
}) {
  useEngineState(session)
  const name = session.engine.document().meta.name
  return (
    <DialogTrigger>
      <Button className="project-button" aria-label={t.projects.open(name)}>
        <span className="project-name">{name}</span>
        <ChevronDown size={16} aria-hidden="true" />
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => <ProjectsPanel session={session} actions={actions} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}

function ProjectsPanel({
  session,
  actions,
  close,
}: {
  session: Session
  actions: ProjectActions
  close: () => void
}) {
  useEngineState(session)
  const doc = session.engine.document()
  const [list, setList] = useState<ProjectSummary[] | null>(null)
  const [deleting, setDeleting] = useState<ProjectSummary | null>(null)
  const restoreInput = useRef<HTMLInputElement>(null)

  useEffect(() => {
    let current = true
    actions.list().then(
      (projects) => current && setList(projects),
      () => current && setList([]),
    )
    return () => {
      current = false
    }
  }, [actions])

  const others = (list ?? []).filter((p) => p.id !== doc.meta.id)
  const run = (task: () => unknown) => {
    close()
    void task()
  }

  return (
    <>
      <Heading slot="title">{t.projects.heading}</Heading>
      <CommitTextField
        key={doc.meta.name}
        label={t.projects.name}
        value={doc.meta.name}
        onCommit={(name) => name && session.dispatch({ type: 'renameProject', name })}
      />
      <div className="button-row">
        <Button className="button" onPress={() => run(() => actions.create())}>
          <FilePlus2 size={18} aria-hidden="true" />
          {t.projects.new}
        </Button>
        <Button className="button" onPress={() => restoreInput.current?.click()}>
          <Upload size={18} aria-hidden="true" />
          {t.projects.restore}
        </Button>
        <input
          ref={restoreInput}
          type="file"
          accept={PROJECT_FILE_TYPES}
          hidden
          data-testid="restore-file"
          onChange={(e) => {
            const file = e.currentTarget.files?.[0]
            e.currentTarget.value = ''
            if (file) run(() => actions.restore(file))
          }}
        />
      </div>

      {deleting ? (
        <div className="confirm" role="alertdialog" aria-labelledby="delete-question">
          <p id="delete-question">
            <strong>{t.projects.deleteQuestion(deleting.name)}</strong>
          </p>
          <p className="muted small">{t.projects.deleteWarning}</p>
          <div className="dialog-actions">
            <Button className="button" onPress={() => setDeleting(null)} autoFocus>
              {t.projects.cancel}
            </Button>
            <Button
              className="button danger"
              onPress={async () => {
                const target = deleting
                setDeleting(null)
                await actions.remove(target)
                setList((l) => l?.filter((p) => p.id !== target.id) ?? null)
              }}
            >
              {t.projects.deleteConfirm}
            </Button>
          </div>
        </div>
      ) : (
        <ul className="project-list" aria-label={t.projects.heading}>
          <li className="project-row current">
            <span className="project-title">{doc.meta.name}</span>
            <span className="muted small">{t.projects.openNow}</span>
          </li>
          {list === null && <li className="muted small">{t.projects.loading}</li>}
          {list !== null && others.length === 0 && (
            <li className="muted small">{t.projects.empty}</li>
          )}
          {others.map((p) => (
            <li key={p.id} className="project-row">
              <Button className="project-open" onPress={() => run(() => actions.open(p.id))}>
                <span className="project-title">{p.name}</span>
                <span className="muted small">{t.projects.summary(p.pieces, p.modified)}</span>
              </Button>
              <Button
                className="icon-button"
                aria-label={t.projects.delete(p.name)}
                onPress={() => setDeleting(p)}
              >
                <Trash2 size={18} aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}

      <div className="dialog-actions">
        <Button className="button" onPress={close}>
          {t.actions.close}
        </Button>
      </div>
    </>
  )
}
