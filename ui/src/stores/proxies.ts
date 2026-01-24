import { create } from 'zustand'
import * as tauri from '../lib/tauri'
import type { SavedProxy } from '../lib/tauri'

interface ProxiesStore {
  proxies: SavedProxy[]
  selectedProxyId: string | null
  isLoading: boolean
  error: string | null

  // Actions
  fetchProxies: () => Promise<void>
  addProxy: (
    name: string,
    host: string,
    port: number,
    username: string | null,
    password: string | null,
  ) => Promise<SavedProxy>
  updateProxy: (
    id: string,
    updates: {
      name?: string
      host?: string
      port?: number
      username?: string | null
      password?: string | null
    },
  ) => Promise<SavedProxy>
  deleteProxy: (id: string) => Promise<void>
  reorderProxies: (ids: string[]) => Promise<void>
  selectProxy: (id: string | null) => void
  setProxies: (proxies: SavedProxy[]) => void
  clearError: () => void
}

export const useProxiesStore = create<ProxiesStore>((set) => ({
  proxies: [],
  selectedProxyId: null,
  isLoading: false,
  error: null,

  fetchProxies: async () => {
    set({ isLoading: true, error: null })
    try {
      const proxies = await tauri.getProxies()
      set({ proxies, isLoading: false })
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  addProxy: async (name, host, port, username, password) => {
    set({ isLoading: true, error: null })
    try {
      const proxy = await tauri.addProxy(name, host, port, username, password)
      // Proxies will be updated via event
      set({ isLoading: false })
      return proxy
    } catch (e) {
      set({ error: String(e), isLoading: false })
      throw e
    }
  },

  updateProxy: async (id, updates) => {
    set({ isLoading: true, error: null })
    try {
      const proxy = await tauri.updateProxy(
        id,
        updates.name,
        updates.host,
        updates.port,
        updates.username,
        updates.password,
      )
      set({ isLoading: false })
      return proxy
    } catch (e) {
      set({ error: String(e), isLoading: false })
      throw e
    }
  },

  deleteProxy: async (id) => {
    set({ isLoading: true, error: null })
    try {
      await tauri.deleteProxy(id)
      // Proxies will be updated via event
      set({ isLoading: false })
    } catch (e) {
      set({ error: String(e), isLoading: false })
      throw e
    }
  },

  reorderProxies: async (ids) => {
    try {
      await tauri.reorderProxies(ids)
      // Proxies will be updated via event
    } catch (e) {
      set({ error: String(e) })
    }
  },

  selectProxy: (id) => {
    set({ selectedProxyId: id })
  },

  setProxies: (proxies) => {
    set({ proxies })
  },

  clearError: () => {
    set({ error: null })
  },
}))

// Subscribe to Tauri events
let unlistenProxies: (() => void) | null = null

export async function initProxiesListeners() {
  unlistenProxies?.()

  unlistenProxies = await tauri.onProxiesChanged((e) => {
    useProxiesStore.getState().setProxies(e.proxies)
  })

  // Initial fetch
  await useProxiesStore.getState().fetchProxies()
}

export function cleanupProxiesListeners() {
  unlistenProxies?.()
  unlistenProxies = null
}
