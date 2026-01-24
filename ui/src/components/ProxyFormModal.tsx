import { motion } from 'framer-motion'
import { X } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { SavedProxy } from '../lib/tauri'
import { useProxiesStore } from '../stores/proxies'
import { ProxyIdenticon } from './ProxyIdenticon'

interface ProxyFormModalProps {
  proxy?: SavedProxy | null
  onClose: () => void
}

export function ProxyFormModal({ proxy, onClose }: ProxyFormModalProps) {
  const { addProxy, updateProxy, isLoading, error, clearError } =
    useProxiesStore()

  const [name, setName] = useState(proxy?.name || '')
  const [host, setHost] = useState(proxy?.host || '')
  const [port, setPort] = useState(proxy?.port?.toString() || '8080')
  const [username, setUsername] = useState(proxy?.username || '')
  const [password, setPassword] = useState('')
  const [formError, setFormError] = useState<string | null>(null)

  const isEditing = !!proxy

  useEffect(() => {
    clearError()
  }, [clearError])

  const validate = (): boolean => {
    if (!name.trim()) {
      setFormError('Name is required')
      return false
    }
    if (!host.trim()) {
      setFormError('Host is required')
      return false
    }
    const portNum = Number.parseInt(port, 10)
    if (Number.isNaN(portNum) || portNum < 1 || portNum > 65535) {
      setFormError('Port must be between 1 and 65535')
      return false
    }
    setFormError(null)
    return true
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!validate()) return

    try {
      const portNum = Number.parseInt(port, 10)
      const usernameVal = username.trim() || null
      const passwordVal = password || null

      if (isEditing) {
        await updateProxy(proxy.id, {
          name: name.trim(),
          host: host.trim(),
          port: portNum,
          username: usernameVal,
          password: passwordVal,
        })
      } else {
        await addProxy(
          name.trim(),
          host.trim(),
          portNum,
          usernameVal,
          passwordVal,
        )
      }
      onClose()
    } catch {
      // Error handled in store
    }
  }

  const previewAddress = `${host || 'example.com'}:${port || '8080'}`

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <motion.div
        initial={{ scale: 0.95, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.95, opacity: 0 }}
        className="w-full max-w-md mx-4 bg-background border border-border rounded-sm shadow-sm"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 className="text-lg font-semibold text-text">
            {isEditing ? 'Edit Proxy' : 'Add Proxy'}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-sm hover:bg-surface transition-colors"
          >
            <X size={18} className="text-text-secondary" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          {/* Icon Preview */}
          <div className="flex items-center gap-3">
            <ProxyIdenticon address={previewAddress} size={48} />
            <p className="text-sm text-text-secondary">
              Icon preview based on address
            </p>
          </div>

          {/* Name */}
          <div>
            <label
              htmlFor="proxy-name"
              className="block text-sm font-medium text-text mb-1"
            >
              Name
            </label>
            <input
              id="proxy-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Proxy"
              className="w-full px-3 py-2 rounded-sm border border-border
                bg-surface text-text
                focus:outline-none focus:border-accent"
            />
          </div>

          {/* Host & Port */}
          <div className="flex gap-3">
            <div className="flex-1">
              <label
                htmlFor="proxy-host"
                className="block text-sm font-medium text-text mb-1"
              >
                Host
              </label>
              <input
                id="proxy-host"
                type="text"
                value={host}
                onChange={(e) => setHost(e.target.value)}
                placeholder="proxy.example.com"
                className="w-full px-3 py-2 rounded-sm border border-border
                  bg-surface text-text
                  focus:outline-none focus:border-accent"
              />
            </div>
            <div className="w-24">
              <label
                htmlFor="proxy-port"
                className="block text-sm font-medium text-text mb-1"
              >
                Port
              </label>
              <input
                id="proxy-port"
                type="number"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                placeholder="8080"
                min="1"
                max="65535"
                className="w-full px-3 py-2 rounded-sm border border-border
                  bg-surface text-text
                  focus:outline-none focus:border-accent"
              />
            </div>
          </div>

          {/* Username */}
          <div>
            <label
              htmlFor="proxy-username"
              className="block text-sm font-medium text-text mb-1"
            >
              Username (optional)
            </label>
            <input
              id="proxy-username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="username"
              className="w-full px-3 py-2 rounded-sm border border-border
                bg-surface text-text
                focus:outline-none focus:border-accent"
            />
          </div>

          {/* Password */}
          <div>
            <label
              htmlFor="proxy-password"
              className="block text-sm font-medium text-text mb-1"
            >
              Password {isEditing && '(leave empty to keep current)'}
            </label>
            <input
              id="proxy-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={isEditing ? '••••••••' : 'password'}
              className="w-full px-3 py-2 rounded-sm border border-border
                bg-surface text-text
                focus:outline-none focus:border-accent"
            />
          </div>

          {/* Error */}
          {(formError || error) && (
            <p className="text-sm text-error">{formError || error}</p>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-sm border border-border
                text-sm font-medium hover:bg-surface transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading}
              className="px-4 py-2 rounded-sm bg-accent text-white
                text-sm font-medium hover:bg-accent-hover transition-colors
                disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isLoading ? 'Saving...' : isEditing ? 'Save' : 'Add'}
            </button>
          </div>
        </form>
      </motion.div>
    </motion.div>
  )
}
