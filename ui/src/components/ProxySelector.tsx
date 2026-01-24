import { AnimatePresence, motion } from 'framer-motion'
import { ChevronDown, ChevronRight, Settings2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useProxiesStore } from '../stores/proxies'
import { ProxyIdenticon } from './ProxyIdenticon'

interface ProxySelectorProps {
  onManageClick?: () => void
}

export function ProxySelector({ onManageClick }: ProxySelectorProps) {
  const { proxies, selectedProxyId, selectProxy } = useProxiesStore()
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  const selectedProxy =
    proxies.find((p) => p.id === selectedProxyId) || proxies[0]

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  if (proxies.length === 0) {
    return (
      <button
        type="button"
        onClick={onManageClick}
        className="flex items-center gap-1 text-sm text-text-secondary hover:text-text transition-colors"
      >
        No proxies configured
        <ChevronRight size={14} />
      </button>
    )
  }

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-2 py-1"
      >
        {selectedProxy && (
          <>
            <ProxyIdenticon
              address={`${selectedProxy.host}:${selectedProxy.port}`}
              size={20}
            />
            <span className="text-sm font-medium text-text max-w-32 truncate">
              {selectedProxy.name}
            </span>
          </>
        )}
        <ChevronDown
          size={16}
          className={`text-text-secondary transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
            transition={{ duration: 0.15 }}
            className="absolute top-full left-1/2 -translate-x-1/2 mt-3 w-56 py-1 z-50
              bg-surface border border-border
              rounded-sm shadow-sm"
          >
            {proxies.map((proxy) => (
              <button
                type="button"
                key={proxy.id}
                onClick={() => {
                  selectProxy(proxy.id)
                  setIsOpen(false)
                }}
                className={`
                  w-full flex items-center gap-2 px-3 py-2 text-left
                  hover:bg-border/50 transition-colors
                  ${proxy.id === selectedProxyId ? 'bg-accent/10' : ''}
                `}
              >
                <ProxyIdenticon
                  address={`${proxy.host}:${proxy.port}`}
                  size={20}
                />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-text truncate">
                    {proxy.name}
                  </p>
                  <p className="text-xs text-text-secondary truncate">
                    {proxy.host}:{proxy.port}
                  </p>
                </div>
              </button>
            ))}

            {/* Manage Proxies */}
            <div className="border-t border-border mt-1 pt-1">
              <button
                type="button"
                onClick={() => {
                  setIsOpen(false)
                  onManageClick?.()
                }}
                className="w-full flex items-center gap-2 px-3 py-2 text-left
                  hover:bg-border/50 transition-colors"
              >
                <Settings2 size={16} className="text-text-secondary" />
                <span className="text-sm text-text-secondary">
                  Manage Proxies
                </span>
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
