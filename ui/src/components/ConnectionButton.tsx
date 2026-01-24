import { motion } from 'framer-motion'
import { Power } from 'lucide-react'
import { useConnectionStore } from '../stores/connection'
import { useProxiesStore } from '../stores/proxies'

export function ConnectionButton() {
  const { state, isLoading, connect, disconnect } = useConnectionStore()
  const { selectedProxyId, proxies } = useProxiesStore()

  const isConnected = state.status === 'connected'
  const isConnecting = state.status === 'connecting'
  const isDisconnecting = state.status === 'disconnecting'
  const hasError = state.status === 'error'
  const isBusy = isLoading || isConnecting || isDisconnecting

  const handleClick = async () => {
    if (isBusy) return

    if (isConnected) {
      await disconnect()
    } else {
      const proxyId = selectedProxyId || proxies[0]?.id
      if (proxyId) {
        await connect(proxyId)
      }
    }
  }

  const canConnect = !isBusy && (selectedProxyId || proxies.length > 0)

  return (
    <div className="flex flex-col items-center gap-4">
      <motion.button
        type="button"
        onClick={handleClick}
        disabled={!canConnect && !isConnected}
        className={`
          relative w-32 h-32 rounded-full
          flex items-center justify-center
          border-4 transition-colors duration-300
          ${
            isConnected
              ? 'border-success bg-success/10 animate-connected'
              : isConnecting
                ? 'border-warning bg-warning/10 animate-connecting'
                : hasError
                  ? 'border-error bg-error/10'
                  : 'border-border bg-surface hover:border-accent'
          }
          disabled:opacity-50 disabled:cursor-not-allowed
        `}
        whileHover={!isBusy && !isConnected ? { scale: 1.05 } : {}}
        whileTap={!isBusy ? { scale: 0.95 } : {}}
      >
        <Power
          size={48}
          className={`
            transition-colors duration-300
            ${
              isConnected
                ? 'text-success'
                : isConnecting
                  ? 'text-warning'
                  : hasError
                    ? 'text-error'
                    : 'text-text-secondary'
            }
          `}
        />
      </motion.button>

      <div className="text-center h-12 flex flex-col justify-center">
        <p className="text-base font-semibold text-text">
          {isConnecting
            ? 'Connecting...'
            : isDisconnecting
              ? 'Disconnecting...'
              : isConnected
                ? 'Connected'
                : hasError
                  ? 'Connection Error'
                  : 'Disconnected'}
        </p>
        {hasError && state.error_message ? (
          <p className="text-xs text-error mt-0.5 max-w-48 truncate">
            {state.error_message}
          </p>
        ) : (
          <p className="text-xs text-text-secondary mt-0.5 h-4 truncate">
            {state.proxy_name && (isConnected || isConnecting)
              ? state.proxy_name
              : '\u00A0'}
          </p>
        )}
      </div>
    </div>
  )
}
