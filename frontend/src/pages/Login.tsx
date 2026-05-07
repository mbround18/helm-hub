import { useState } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { authApi } from '../lib/api'
import { useAuthStore } from '../stores/auth'
import { Package, AlertCircle } from 'lucide-react'

export function Login() {
  const [form, setForm] = useState({ username: '', password: '', totp_code: '' })
  const [needsTotp, setNeedsTotp] = useState(false)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const { setAuth } = useAuthStore()
  const navigate = useNavigate()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { data } = await authApi.login(
        form.username,
        form.password,
        needsTotp ? form.totp_code : undefined
      )
      setAuth(data.token, data.user)
      navigate(`/u/${data.user.username}`)
    } catch (err: any) {
      const msg: string = err?.response?.data?.error ?? 'Login failed'
      if (msg.toLowerCase().includes('totp')) setNeedsTotp(true)
      setError(msg)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-sm">
        <div className="flex items-center justify-center gap-2 text-violet-400 mb-8">
          <Package className="w-6 h-6" />
          <span className="text-xl font-semibold">Helm Hub</span>
        </div>

        <div className="bg-gray-900 border border-gray-800 rounded-xl p-6">
          <h1 className="text-lg font-semibold text-white mb-5">Sign in</h1>

          {error && (
            <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm mb-4">
              <AlertCircle className="w-4 h-4 shrink-0" />
              {error}
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4">
            <Field
              label="Username"
              type="text"
              value={form.username}
              onChange={(v) => setForm((f) => ({ ...f, username: v }))}
              autoComplete="username"
            />
            <Field
              label="Password"
              type="password"
              value={form.password}
              onChange={(v) => setForm((f) => ({ ...f, password: v }))}
              autoComplete="current-password"
            />
            {needsTotp && (
              <Field
                label="Authenticator code"
                type="text"
                value={form.totp_code}
                onChange={(v) => setForm((f) => ({ ...f, totp_code: v }))}
                placeholder="6-digit code"
                autoComplete="one-time-code"
              />
            )}
            <button
              type="submit"
              disabled={loading}
              className="w-full bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white py-2.5 rounded-lg text-sm font-medium transition-colors"
            >
              {loading ? 'Signing in…' : 'Sign in'}
            </button>
          </form>

          <p className="text-center text-sm text-gray-500 mt-5">
            No account?{' '}
            <Link to="/register" className="text-violet-400 hover:text-violet-300">
              Register
            </Link>
          </p>
        </div>
      </div>
    </div>
  )
}

function Field({
  label,
  type,
  value,
  onChange,
  placeholder,
  autoComplete,
}: {
  label: string
  type: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  autoComplete?: string
}) {
  return (
    <div>
      <label className="block text-xs font-medium text-gray-400 mb-1.5">{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoComplete={autoComplete}
        required
        className="w-full bg-gray-800 border border-gray-700 text-white placeholder-gray-600 rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
      />
    </div>
  )
}
