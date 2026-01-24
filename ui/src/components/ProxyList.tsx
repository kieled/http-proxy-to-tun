import { Reorder } from 'framer-motion'
import { GripVertical, Pencil, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { SavedProxy } from '../lib/tauri'
import { useConnectionStore } from '../stores/connection'
import { useProxiesStore } from '../stores/proxies'
import { ProxyFormModal } from './ProxyFormModal'
import { ProxyIdenticon } from './ProxyIdenticon'

interface ProxyListProps {
  isEditMode: boolean
  onEditModeChange: (mode: boolean) => void
}

export function ProxyList({ isEditMode, onEditModeChange }: ProxyListProps) {
  const { proxies, selectedProxyId, selectProxy, deleteProxy, reorderProxies } =
    useProxiesStore()
  const { state: connectionState, connect } = useConnectionStore()
  const [editingProxy, setEditingProxy] = useState<SavedProxy | null>(null)
  const [showAddModal, setShowAddModal] = useState(false)
  const [localProxies, setLocalProxies] = useState(proxies)

  // Sync local proxies with store when not reordering
  if (!isEditMode && localProxies !== proxies) {
    setLocalProxies(proxies)
  }

  const handleReorder = (newOrder: SavedProxy[]) => {
    setLocalProxies(newOrder)
  }

  const handleReorderEnd = async () => {
    const ids = localProxies.map((p) => p.id)
    await reorderProxies(ids)
  }

  const handleDelete = async (proxy: SavedProxy) => {
    if (confirm(`Delete proxy "${proxy.name}"?`)) {
      try {
        await deleteProxy(proxy.id)
      } catch {
        // Error handled in store
      }
    }
  }

  const handleSelect = async (proxy: SavedProxy) => {
    if (isEditMode) return

    selectProxy(proxy.id)

    // If connected to a different proxy, auto-reconnect
    if (
      connectionState.status === 'connected' &&
      connectionState.proxy_id !== proxy.id
    ) {
      await connect(proxy.id)
    }
  }

  const isConnected = connectionState.status === 'connected'

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto">
        {proxies.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center p-4">
            <p className="text-text-secondary mb-4">
              No proxies configured yet
            </p>
            <button
              type="button"
              onClick={() => setShowAddModal(true)}
              className="flex items-center gap-2 px-4 py-2 rounded-sm
                bg-accent text-white font-medium
                hover:bg-accent-hover transition-colors"
            >
              <Plus size={16} />
              Add Proxy
            </button>
          </div>
        ) : (
          <Reorder.Group
            axis="y"
            values={localProxies}
            onReorder={handleReorder}
            className="flex flex-col"
          >
            {localProxies.map((proxy) => (
              <Reorder.Item
                key={proxy.id}
                value={proxy}
                dragListener={isEditMode}
                onDragEnd={handleReorderEnd}
                className={`
                  flex items-center gap-3 px-4 py-3
                  border-b border-border
                  ${!isEditMode && 'cursor-pointer hover:bg-surface'}
                  ${selectedProxyId === proxy.id && !isEditMode ? 'bg-accent/10' : ''}
                  ${isEditMode ? 'cursor-grab active:cursor-grabbing' : ''}
                `}
                onClick={() => handleSelect(proxy)}
              >
                {isEditMode && (
                  <GripVertical size={16} className="text-muted shrink-0" />
                )}

                <ProxyIdenticon
                  address={`${proxy.host}:${proxy.port}`}
                  size={32}
                />

                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-text truncate">
                    {proxy.name}
                  </p>
                  <p className="text-xs text-text-secondary truncate">
                    {proxy.host}:{proxy.port}
                    {proxy.username && ` (${proxy.username})`}
                  </p>
                </div>

                {isEditMode && (
                  <div className="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation()
                        setEditingProxy(proxy)
                      }}
                      className="p-2 rounded-sm hover:bg-border transition-colors"
                    >
                      <Pencil size={14} className="text-text-secondary" />
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleDelete(proxy)
                      }}
                      disabled={
                        isConnected && connectionState.proxy_id === proxy.id
                      }
                      className="p-2 rounded-sm hover:bg-error/10 transition-colors
                        disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <Trash2 size={14} className="text-error" />
                    </button>
                  </div>
                )}

                {!isEditMode && selectedProxyId === proxy.id && isConnected && (
                  <div className="w-2 h-2 rounded-full bg-success shrink-0" />
                )}
              </Reorder.Item>
            ))}
          </Reorder.Group>
        )}
      </div>

      {proxies.length > 0 && (
        <div className="border-t border-border flex w-full">
          <button
            type="button"
            onClick={() => setShowAddModal(true)}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-3
              text-sm font-medium border-r border-border
              hover:bg-surface transition-colors"
          >
            <Plus size={14} />
            Add Proxy
          </button>
          <button
            type="button"
            onClick={() => onEditModeChange(!isEditMode)}
            className={`
              py-3 text-sm font-medium transition-colors flex justify-center w-28
              ${
                isEditMode
                  ? 'bg-(--color-accent) text-white'
                  : 'hover:bg-surface'
              }
            `}
          >
            {isEditMode ? 'Done' : 'Edit'}
          </button>
        </div>
      )}

      {(showAddModal || editingProxy) && (
        <ProxyFormModal
          proxy={editingProxy}
          onClose={() => {
            setShowAddModal(false)
            setEditingProxy(null)
          }}
        />
      )}
    </div>
  )
}
