import { invoke } from '@tauri-apps/api/core'
import { type UnlistenFn, listen } from '@tauri-apps/api/event'

// Types matching Rust backend
export type Theme = 'light' | 'dark' | 'fulldark' | 'auto'

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'disconnecting'
  | 'error'

export interface SavedProxy {
  id: string
  name: string
  host: string
  port: number
  username: string | null
  order: number
}

export interface ConnectionState {
  status: ConnectionStatus
  proxy_id: string | null
  proxy_name: string | null
  connected_since: number | null
  duration_secs: number
  error_message: string | null
  public_ip: string | null
}

export interface AppSettings {
  theme: Theme
  killswitch: boolean
  last_proxy_id: string | null
  minimize_to_tray: boolean
  auto_connect: boolean
}

// Proxy commands
export async function getProxies(): Promise<SavedProxy[]> {
  return invoke('get_proxies')
}

export async function getProxy(id: string): Promise<SavedProxy | null> {
  return invoke('get_proxy', { id })
}

export async function addProxy(
  name: string,
  host: string,
  port: number,
  username: string | null,
  password: string | null,
): Promise<SavedProxy> {
  return invoke('add_proxy', { name, host, port, username, password })
}

export async function updateProxy(
  id: string,
  name?: string,
  host?: string,
  port?: number,
  username?: string | null,
  password?: string | null,
): Promise<SavedProxy> {
  return invoke('update_proxy', { id, name, host, port, username, password })
}

export async function deleteProxy(id: string): Promise<void> {
  return invoke('delete_proxy', { id })
}

export async function reorderProxies(ids: string[]): Promise<void> {
  return invoke('reorder_proxies', { ids })
}

// Connection commands
export async function getConnectionStatus(): Promise<ConnectionState> {
  return invoke('get_connection_status')
}

export async function connect(proxyId: string): Promise<void> {
  return invoke('connect', { proxyId })
}

export async function disconnect(): Promise<void> {
  return invoke('disconnect')
}

// Settings commands
export async function getSettings(): Promise<AppSettings> {
  return invoke('get_settings')
}

export async function updateSettings(
  settings: AppSettings,
): Promise<AppSettings> {
  return invoke('update_settings', { settings })
}

export async function setTheme(theme: Theme): Promise<void> {
  return invoke('set_theme', { theme })
}

// Privilege check
export async function checkPrivileges(): Promise<boolean> {
  return invoke('check_privileges_command')
}

export async function canElevatePrivileges(): Promise<boolean> {
  return invoke('can_elevate_privileges')
}

export async function setCapabilities(): Promise<void> {
  return invoke('set_capabilities')
}

// Network info
export async function refreshPublicIP(): Promise<string | null> {
  return invoke('refresh_public_ip')
}

// Event listeners
export interface ConnectionStatusEvent {
  state: ConnectionState
}

export interface ConnectionTimeEvent {
  duration_secs: number
}

export interface ProxiesChangedEvent {
  proxies: SavedProxy[]
}

export function onConnectionStatus(
  callback: (event: ConnectionStatusEvent) => void,
): Promise<UnlistenFn> {
  return listen<ConnectionStatusEvent>('connection-status', (e) =>
    callback(e.payload),
  )
}

export function onConnectionTime(
  callback: (event: ConnectionTimeEvent) => void,
): Promise<UnlistenFn> {
  return listen<ConnectionTimeEvent>('connection-time', (e) =>
    callback(e.payload),
  )
}

export function onProxiesChanged(
  callback: (event: ProxiesChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProxiesChangedEvent>('proxies-changed', (e) =>
    callback(e.payload),
  )
}

// Tray events
export function onTrayConnect(callback: () => void): Promise<UnlistenFn> {
  return listen('tray-connect', callback)
}

export function onTrayDisconnect(callback: () => void): Promise<UnlistenFn> {
  return listen('tray-disconnect', callback)
}
