import { AlertTriangle, ArrowLeft, Settings } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { ConnectionButton } from './components/ConnectionButton'
import { ProxyList } from './components/ProxyList'
import { ProxySelector } from './components/ProxySelector'
import { SettingsView } from './components/SettingsView'
import { StatusBar } from './components/StatusBar'
import { Titlebar } from './components/Titlebar'
import {
  canElevatePrivileges,
  checkPrivileges,
  onTrayConnect,
  onTrayDisconnect,
  setCapabilities,
} from './lib/tauri'
import {
  cleanupConnectionListeners,
  initConnectionListeners,
  useConnectionStore,
} from './stores/connection'
import {
  cleanupProxiesListeners,
  initProxiesListeners,
  useProxiesStore,
} from './stores/proxies'
import { initSettingsListeners } from './stores/settings'

type View = 'main' | 'proxies' | 'settings'

export default function App() {
  const [view, setView] = useState<View>('main')
  const [isEditMode, setIsEditMode] = useState(false)
  const [hasPrivileges, setHasPrivileges] = useState<boolean | null>(null)
  const [canElevate, setCanElevate] = useState(false)
  const [isElevating, setIsElevating] = useState(false)
  const { connect, disconnect } = useConnectionStore()
  const { selectedProxyId, proxies } = useProxiesStore()

  // Use refs for tray handlers to avoid stale closures
  const connectRef = useRef(connect)
  const disconnectRef = useRef(disconnect)
  const selectedProxyIdRef = useRef(selectedProxyId)
  const proxiesRef = useRef(proxies)

  useEffect(() => {
    connectRef.current = connect
    disconnectRef.current = disconnect
    selectedProxyIdRef.current = selectedProxyId
    proxiesRef.current = proxies
  }, [connect, disconnect, selectedProxyId, proxies])

  const handleElevate = async () => {
    setIsElevating(true)
    try {
      await setCapabilities()
      // Re-check privileges after setting capabilities
      const privs = await checkPrivileges()
      setHasPrivileges(privs)
    } catch (e) {
      console.error('Failed to elevate:', e)
    } finally {
      setIsElevating(false)
    }
  }

  // Initialize stores and event listeners
  useEffect(() => {
    const init = async () => {
      // Check privileges
      const [privs, canElev] = await Promise.all([
        checkPrivileges(),
        canElevatePrivileges(),
      ])
      setHasPrivileges(privs)
      setCanElevate(canElev)

      // Initialize stores
      await Promise.all([
        initConnectionListeners(),
        initProxiesListeners(),
        initSettingsListeners(),
      ])

      // Setup tray event handlers
      const unlistenConnect = await onTrayConnect(async () => {
        const proxyId = selectedProxyIdRef.current || proxiesRef.current[0]?.id
        if (proxyId) {
          await connectRef.current(proxyId)
        }
      })

      const unlistenDisconnect = await onTrayDisconnect(async () => {
        await disconnectRef.current()
      })

      return () => {
        unlistenConnect()
        unlistenDisconnect()
      }
    }

    const cleanup = init()

    return () => {
      cleanup.then((fn) => fn?.())
      cleanupConnectionListeners()
      cleanupProxiesListeners()
    }
  }, [])

  // Privilege warning banner (non-blocking for dev)
  const showPrivilegeWarning = hasPrivileges === false

  return (
    <div className="flex flex-col h-screen bg-background">
      {/* Privilege warning banner */}
      {showPrivilegeWarning && (
        <div className="flex items-center justify-between gap-2 px-4 py-2 bg-warning/10 border-b border-warning/30">
          <div className="flex items-center gap-2">
            <AlertTriangle size={14} className="text-warning" />
            <p className="text-xs text-warning">Missing CAP_NET_ADMIN</p>
          </div>
          {canElevate && (
            <button
              type="button"
              onClick={handleElevate}
              disabled={isElevating}
              className="text-xs px-2 py-1 rounded-sm bg-warning text-white
                hover:opacity-90 transition-opacity disabled:opacity-50"
            >
              {isElevating ? 'Granting...' : 'Grant Access'}
            </button>
          )}
        </div>
      )}

      {view === 'settings' ? (
        <>
          <Titlebar
            left={
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setView('main')}
                  className="p-1 rounded-sm hover:bg-surface transition-colors"
                >
                  <ArrowLeft size={16} className="text-text" />
                </button>
                <span className="text-sm font-medium text-text">Settings</span>
              </div>
            }
          />
          <SettingsView onBack={() => setView('main')} />
        </>
      ) : view === 'proxies' ? (
        <div className="flex flex-col h-full">
          <Titlebar
            left={
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setView('main')
                    setIsEditMode(false)
                  }}
                  className="p-1 rounded-sm hover:bg-surface transition-colors"
                >
                  <ArrowLeft size={16} className="text-text" />
                </button>
                <span className="text-sm font-medium text-text">Proxies</span>
              </div>
            }
          />
          <ProxyList isEditMode={isEditMode} onEditModeChange={setIsEditMode} />
        </div>
      ) : (
        <>
          <Titlebar
            left={
              <button
                type="button"
                onClick={() => setView('settings')}
                className="p-2 rounded-sm transition-colors group"
              >
                <Settings
                  size={16}
                  className="text-text-secondary group-hover:text-text"
                />
              </button>
            }
            center={<ProxySelector onManageClick={() => setView('proxies')} />}
          />

          {/* Main Content */}
          <main className="flex-1 flex flex-col items-center justify-center p-4">
            <ConnectionButton />
          </main>

          {/* Status Bar at bottom */}
          <StatusBar />
        </>
      )}
    </div>
  )
}
