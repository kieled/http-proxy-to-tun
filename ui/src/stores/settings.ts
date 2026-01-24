import { create } from 'zustand'
import * as tauri from '../lib/tauri'
import type { AppSettings, Theme } from '../lib/tauri'

interface SettingsStore {
  settings: AppSettings
  isLoading: boolean
  error: string | null

  // Actions
  fetchSettings: () => Promise<void>
  updateSettings: (settings: Partial<AppSettings>) => Promise<void>
  setTheme: (theme: Theme) => Promise<void>
  applyTheme: (theme: Theme) => void
  clearError: () => void
}

const defaultSettings: AppSettings = {
  theme: 'light',
  killswitch: true,
  last_proxy_id: null,
  close_to_tray: true,
  auto_connect: false,
}

function getSystemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light'
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light'
}

function applyThemeToDocument(theme: Theme) {
  const root = document.documentElement

  // Remove all theme classes
  root.classList.remove('theme-light', 'theme-dark', 'theme-full-dark')

  // Apply new theme
  if (theme === 'auto') {
    const systemTheme = getSystemTheme()
    root.classList.add(`theme-${systemTheme}`)
  } else if (theme === 'fulldark') {
    root.classList.add('theme-full-dark')
  } else {
    root.classList.add(`theme-${theme}`)
  }
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  settings: defaultSettings,
  isLoading: false,
  error: null,

  fetchSettings: async () => {
    set({ isLoading: true, error: null })
    try {
      const settings = await tauri.getSettings()
      set({ settings, isLoading: false })
      get().applyTheme(settings.theme)
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  updateSettings: async (updates) => {
    const currentSettings = get().settings
    const newSettings = { ...currentSettings, ...updates }

    set({ isLoading: true, error: null })
    try {
      const settings = await tauri.updateSettings(newSettings)
      set({ settings, isLoading: false })
      get().applyTheme(settings.theme)
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  setTheme: async (theme) => {
    try {
      await tauri.setTheme(theme)
      set((s) => ({ settings: { ...s.settings, theme } }))
      get().applyTheme(theme)
    } catch (e) {
      set({ error: String(e) })
    }
  },

  applyTheme: (theme) => {
    applyThemeToDocument(theme)
  },

  clearError: () => {
    set({ error: null })
  },
}))

// Listen for system theme changes
if (typeof window !== 'undefined') {
  window
    .matchMedia('(prefers-color-scheme: dark)')
    .addEventListener('change', () => {
      const { settings, applyTheme } = useSettingsStore.getState()
      if (settings.theme === 'auto') {
        applyTheme('auto')
      }
    })
}

export async function initSettingsListeners() {
  await useSettingsStore.getState().fetchSettings()
}
