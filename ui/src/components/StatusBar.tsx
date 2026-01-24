import { Clock, Globe } from 'lucide-react'
import { useConnectionStore } from '../stores/connection'

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60

  const pad = (n: number) => n.toString().padStart(2, '0')

  return `${pad(hours)}:${pad(minutes)}:${pad(secs)}`
}

export function StatusBar() {
  const { state } = useConnectionStore()

  const isConnected = state.status === 'connected'

  return (
    <div className="flex items-center justify-center gap-6 py-3 px-4 border-t border-border bg-surface/50">
      {/* IP Address */}
      <div className="flex items-center gap-2 min-w-35 justify-center">
        <Globe size={14} className="text-text-secondary shrink-0" />
        <span className="text-xs font-mono text-text-secondary tabular-nums">
          {state.public_ip ? (
            state.public_ip
          ) : (
            <span className="opacity-50">---.---.---.---</span>
          )}
        </span>
      </div>

      {/* Separator */}
      <div className="w-px h-4 bg-border" />

      {/* Connection Time */}
      <div className="flex items-center gap-2 min-w-25 justify-center">
        <Clock size={14} className="text-text-secondary shrink-0" />
        <span
          className={`text-xs font-mono tabular-nums ${
            isConnected ? 'text-success' : 'text-text-secondary'
          }`}
        >
          {isConnected ? formatDuration(state.duration_secs) : '00:00:00'}
        </span>
      </div>
    </div>
  )
}
