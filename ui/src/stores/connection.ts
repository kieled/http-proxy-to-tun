import { create } from 'zustand'
import * as tauri from '../lib/tauri'
import type { ConnectionState } from '../lib/tauri'

interface ConnectionStore {
  state: ConnectionState
  isLoading: boolean
  error: string | null

  // Actions
  fetchStatus: () => Promise<void>
  connect: (proxyId: string) => Promise<void>
  disconnect: () => Promise<void>
  updateState: (state: ConnectionState) => void
  updateDuration: (durationSecs: number) => void
  clearError: () => void
}

const initialState: ConnectionState = {
  status: 'disconnected',
  proxy_id: null,
  proxy_name: null,
  connected_since: null,
  duration_secs: 0,
  error_message: null,
  public_ip: null,
}

export const useConnectionStore = create<ConnectionStore>((set) => ({
  state: initialState,
  isLoading: false,
  error: null,

  fetchStatus: async () => {
    try {
      const state = await tauri.getConnectionStatus()
      set({ state, error: null })
    } catch (e) {
      set({ error: String(e) })
    }
  },

  connect: async (proxyId: string) => {
    set({ isLoading: true, error: null })
    try {
      await tauri.connect(proxyId)
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isLoading: false })
    }
  },

  disconnect: async () => {
    set({ isLoading: true, error: null })
    try {
      await tauri.disconnect()
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isLoading: false })
    }
  },

  updateState: (state: ConnectionState) => {
    set({ state })
  },

  updateDuration: (durationSecs: number) => {
    set((s) => ({
      state: { ...s.state, duration_secs: durationSecs },
    }))
  },

  clearError: () => {
    set({ error: null })
  },
}))

// Subscribe to Tauri events
let unlistenStatus: (() => void) | null = null
let unlistenTime: (() => void) | null = null

export async function initConnectionListeners() {
  // Clean up existing listeners
  unlistenStatus?.()
  unlistenTime?.()

  unlistenStatus = await tauri.onConnectionStatus((e) => {
    useConnectionStore.getState().updateState(e.state)
  })

  unlistenTime = await tauri.onConnectionTime((e) => {
    useConnectionStore.getState().updateDuration(e.duration_secs)
  })

  // Initial fetch
  await useConnectionStore.getState().fetchStatus()

  // Fetch initial public IP (async, updates state via event)
  tauri.refreshPublicIP().catch(() => {})
}

export function cleanupConnectionListeners() {
  unlistenStatus?.()
  unlistenTime?.()
  unlistenStatus = null
  unlistenTime = null
}
