import { getCurrentWindow } from '@tauri-apps/api/window'
import { X } from 'lucide-react'

interface TitlebarProps {
  left?: React.ReactNode
  center?: React.ReactNode
  right?: React.ReactNode
}

export function Titlebar({ left, center, right }: TitlebarProps) {
  const appWindow = getCurrentWindow()

  const handleClose = () => {
    appWindow.close()
  }

  return (
    <div
      data-tauri-drag-region
      className="grid grid-cols-[1fr_auto_1fr] items-center h-10 px-3 border-b border-border bg-background select-none"
    >
      {/* Left side */}
      <div className="flex items-center gap-2 min-w-0 justify-start">
        {left}
      </div>

      {/* Center */}
      <div className="flex items-center justify-center">{center}</div>

      {/* Right side - App actions + Window controls */}
      <div className="flex items-center gap-1 justify-end -mr-1">
        {right}
        <button
          type="button"
          onClick={handleClose}
          className="p-2 rounded-sm hover:bg-error/10 transition-colors group"
          title="Close"
        >
          <X size={14} className="text-text-secondary group-hover:text-error" />
        </button>
      </div>
    </div>
  )
}
