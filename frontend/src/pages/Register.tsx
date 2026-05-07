import { useState } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { authApi } from '../lib/api'
import { Package, AlertCircle } from 'lucide-react'

export function Register() {
  const [form, setForm] = useState({ username: '', email: '', password: '' })
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()

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
        <div className="flex items-center justify-center gap-2 text-violet-400 mb-8">
          <Package className="w-6 h-6" />
          <span className="text-xl font-semibold">Helm Hub</span>
        </div>

        <div className="bg-gray-900 border border-gray-800 rounded-xl p-6">
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
        </div>
      </div>
    </div>
  )
}
