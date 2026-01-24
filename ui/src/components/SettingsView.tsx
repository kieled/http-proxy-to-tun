import { MonitorDown, Shield, Zap } from 'lucide-react'
import { useSettingsStore } from '../stores/settings'
import { ThemeSelector } from './ThemeSelector'

interface SettingsViewProps {
  onBack: () => void
}

export function SettingsView({ onBack: _ }: SettingsViewProps) {
  const { settings, updateSettings } = useSettingsStore()

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-6">
      {/* Theme */}
      <div>
        <h2 className="text-sm font-medium text-text mb-2">Theme</h2>
        <ThemeSelector />
      </div>

      {/* Killswitch */}
      <div className="flex items-center justify-between py-2">
        <div className="flex items-center gap-3">
          <Shield size={18} className="text-text-secondary" />
          <div>
            <p className="text-sm font-medium text-text">Killswitch</p>
            <p className="text-xs text-text-secondary">
              Block all traffic when disconnected unexpectedly
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={() => updateSettings({ killswitch: !settings.killswitch })}
          className={`
              relative w-11 h-6 rounded-full transition-colors
              ${settings.killswitch ? 'bg-accent' : 'bg-border'}
            `}
        >
          <div
            className={`
                absolute top-1 w-4 h-4 rounded-full bg-white transition-transform
                ${settings.killswitch ? 'left-6' : 'left-1'}
              `}
          />
        </button>
      </div>

      {/* Close to Tray */}
      <div className="flex items-center justify-between py-2">
        <div className="flex items-center gap-3">
          <MonitorDown size={18} className="text-text-secondary" />
          <div>
            <p className="text-sm font-medium text-text">Close to Tray</p>
            <p className="text-xs text-text-secondary">
              Keep running in system tray when closed
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={() =>
            updateSettings({ close_to_tray: !settings.close_to_tray })
          }
          className={`
              relative w-11 h-6 rounded-full transition-colors
              ${settings.close_to_tray ? 'bg-accent' : 'bg-border'}
            `}
        >
          <div
            className={`
                absolute top-1 w-4 h-4 rounded-full bg-white transition-transform
                ${settings.close_to_tray ? 'left-6' : 'left-1'}
              `}
          />
        </button>
      </div>

      {/* Auto Connect */}
      <div className="flex items-center justify-between py-2">
        <div className="flex items-center gap-3">
          <Zap size={18} className="text-text-secondary" />
          <div>
            <p className="text-sm font-medium text-text">Auto Connect</p>
            <p className="text-xs text-text-secondary">
              Connect to last proxy on startup
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={() =>
            updateSettings({ auto_connect: !settings.auto_connect })
          }
          className={`
              relative w-11 h-6 rounded-full transition-colors
              ${settings.auto_connect ? 'bg-accent' : 'bg-border'}
            `}
        >
          <div
            className={`
                absolute top-1 w-4 h-4 rounded-full bg-white transition-transform
                ${settings.auto_connect ? 'left-6' : 'left-1'}
              `}
          />
        </button>
      </div>

      {/* About */}
      <div className="pt-4 border-t border-border">
        <p className="text-xs text-text-secondary text-center">
          HTTP Tunnel v0.1.0
        </p>
      </div>
    </div>
  )
}
