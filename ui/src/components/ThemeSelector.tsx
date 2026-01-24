import { Circle, Monitor, Moon, Sun } from 'lucide-react'
import type { Theme } from '../lib/tauri'
import { useSettingsStore } from '../stores/settings'

const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'fulldark', label: 'OLED', icon: Circle },
  { value: 'auto', label: 'Auto', icon: Monitor },
]

export function ThemeSelector() {
  const { settings, setTheme } = useSettingsStore()

  return (
    <div className="flex rounded-md border border-border overflow-hidden">
      {themes.map(({ value, label, icon: Icon }, index) => (
        <button
          type="button"
          key={value}
          onClick={() => setTheme(value)}
          className={`
            flex-1 flex items-center justify-center gap-1.5 px-3 py-2 text-sm
            transition-colors
            ${index > 0 ? 'border-l border-border' : ''}
            ${
              settings.theme === value
                ? 'bg-accent text-white'
                : 'bg-surface text-text-secondary hover:bg-border/50 hover:text-text'
            }
          `}
        >
          <Icon size={14} />
          <span>{label}</span>
        </button>
      ))}
    </div>
  )
}
