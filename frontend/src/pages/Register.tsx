import { useState } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { authApi } from '../lib/api'
import { AlertCircle, Lock } from 'lucide-react'
import { AppLogo } from '../components/AppLogo'
import { useSettings } from '../hooks/useSettings'

export function Register() {
  const [form, setForm] = useState({ username: '', email: '', password: '' })
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const { signup_enabled } = useSettings()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      await authApi.register(form.username, form.email, form.password)
      navigate('/login')
    } catch (err: any) {
      setError(err?.response?.data?.error ?? 'Registration failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-sm">
        <div className="flex items-center justify-center text-violet-400 mb-8">
          <AppLogo size="md" />
        </div>

        <div className="bg-gray-900 border border-gray-800 rounded-xl p-6">
          {!signup_enabled ? (
            <div className="text-center py-4">
              <Lock className="w-8 h-8 text-gray-600 mx-auto mb-3" />
              <h1 className="text-lg font-semibold text-white mb-2">Registration closed</h1>
              <p className="text-sm text-gray-500 mb-5">
                New account registration is currently disabled by the administrator.
              </p>
              <Link to="/login" className="text-sm text-violet-400 hover:text-violet-300">
                Back to sign in
              </Link>
            </div>
          ) : (
            <>
              <h1 className="text-lg font-semibold text-white mb-5">Create account</h1>

              {error && (
                <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm mb-4">
                  <AlertCircle className="w-4 h-4 shrink-0" />
                  {error}
                </div>
              )}

              <form onSubmit={handleSubmit} className="space-y-4">
                {(['username', 'email', 'password'] as const).map((field) => (
                  <div key={field}>
                    <label className="block text-xs font-medium text-gray-400 mb-1.5 capitalize">
                      {field}
                    </label>
                    <input
                      type={field === 'password' ? 'password' : field === 'email' ? 'email' : 'text'}
                      value={form[field]}
                      onChange={(e) => setForm((f) => ({ ...f, [field]: e.target.value }))}
                      required
                      className="w-full bg-gray-800 border border-gray-700 text-white placeholder-gray-600 rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
                    />
                  </div>
                ))}
                <button
                  type="submit"
                  disabled={loading}
                  className="w-full bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white py-2.5 rounded-lg text-sm font-medium transition-colors"
                >
                  {loading ? 'Creating account…' : 'Create account'}
                </button>
              </form>

              <p className="text-center text-sm text-gray-500 mt-5">
                Already have an account?{' '}
                <Link to="/login" className="text-violet-400 hover:text-violet-300">
                  Sign in
                </Link>
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
