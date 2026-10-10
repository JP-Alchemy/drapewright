import { MousePointer2, MoveVertical } from 'lucide-react'
import { ToggleButton, ToggleButtonGroup } from 'react-aria-components'
import type { Session, ToolId } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'

/** A notch: a short cut across an edge. */
function NotchIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M3 15h18" />
      <path d="M12 15V7" />
    </svg>
  )
}

const TOOLS: { id: ToolId; label: string; icon: React.ReactNode }[] = [
  { id: 'select', label: t.toolbar.select, icon: <MousePointer2 size={20} aria-hidden="true" /> },
  { id: 'notch', label: t.toolbar.notch, icon: <NotchIcon /> },
  {
    id: 'grainline',
    label: t.toolbar.grainline,
    icon: <MoveVertical size={20} aria-hidden="true" />,
  },
]

/** The tools, down the left of the table, with a hint for the active one. */
export function Toolbar({ session }: { session: Session }) {
  const { tool } = useStore(session.ui)
  return (
    <div className="toolbar-wrap">
      <ToggleButtonGroup
        className="toolbar"
        aria-label={t.toolbar.label}
        orientation="vertical"
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={[tool]}
        onSelectionChange={(keys) => {
          const next = [...keys][0]
          if (next) session.setTool(next as ToolId)
        }}
      >
        {TOOLS.map(({ id, label, icon }) => (
          <ToggleButton key={id} id={id} className="tool-button" aria-label={label}>
            {icon}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>
      <p className="tool-hint" role="status">
        {t.toolbar.hints[tool]}
      </p>
    </div>
  )
}
