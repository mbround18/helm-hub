import { Suspense, lazy } from 'react'
import { BrowserRouter, Routes, Route, Navigate, Outlet } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useAuthStore } from './stores/auth'

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, staleTime: 30_000 } },
})

const Shell = lazy(async () => ({ default: (await import('./components/layout/Shell')).Shell }))
const Explore = lazy(async () => ({ default: (await import('./pages/Explore')).Explore }))
const Dashboard = lazy(async () => ({ default: (await import('./pages/Dashboard')).Dashboard }))
const Profile = lazy(async () => ({ default: (await import('./pages/Profile')).Profile }))
const Login = lazy(async () => ({ default: (await import('./pages/Login')).Login }))
const Register = lazy(async () => ({ default: (await import('./pages/Register')).Register }))

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuthStore()
  return isAuthenticated() ? <>{children}</> : <Navigate to="/login" replace />
}

function AuthPageFallback() {
  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-sm rounded-xl border border-gray-800 bg-gray-900 p-6">
        <div className="space-y-3 animate-pulse">
          <div className="h-6 w-32 rounded bg-gray-800" />
          <div className="h-10 rounded-lg bg-gray-800" />
          <div className="h-10 rounded-lg bg-gray-800" />
          <div className="h-10 rounded-lg bg-gray-800" />
        </div>
      </div>
    </div>
  )
}

function ShellFallback() {
  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex">
      <aside className="w-60 shrink-0 bg-gray-900 border-r border-gray-800 p-5">
        <div className="h-6 w-28 rounded bg-gray-800 animate-pulse" />
        <div className="mt-8 space-y-2">
          <div className="h-10 rounded-md bg-gray-800 animate-pulse" />
          <div className="h-10 rounded-md bg-gray-800 animate-pulse" />
        </div>
      </aside>
      <main className="flex-1 p-6">
        <div className="space-y-4 animate-pulse">
          <div className="h-8 w-48 rounded bg-gray-800" />
          <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
            <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
          </div>
        </div>
      </main>
    </div>
  )
}

function PageFallback() {
  return (
    <div className="p-6 max-w-6xl mx-auto">
      <div className="space-y-4 animate-pulse">
        <div className="h-8 w-40 rounded bg-gray-800" />
        <div className="h-12 rounded-xl bg-gray-900 border border-gray-800" />
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
          <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
          <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
          <div className="h-24 rounded-xl bg-gray-900 border border-gray-800" />
        </div>
      </div>
    </div>
  )
}

function ShellLayout() {
  return (
    <Suspense fallback={<ShellFallback />}>
      <Shell>
        <Suspense fallback={<PageFallback />}>
          <Outlet />
        </Suspense>
      </Shell>
    </Suspense>
  )
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route
            path="/login"
            element={
              <Suspense fallback={<AuthPageFallback />}>
                <Login />
              </Suspense>
            }
          />
          <Route
            path="/register"
            element={
              <Suspense fallback={<AuthPageFallback />}>
                <Register />
              </Suspense>
            }
          />
          <Route element={<ShellLayout />}>
            <Route index element={<Explore />} />
            <Route
              path="/u/:username"
              element={
                <ProtectedRoute>
                  <Dashboard />
                </ProtectedRoute>
              }
            />
            <Route
              path="/profile"
              element={
                <ProtectedRoute>
                  <Profile />
                </ProtectedRoute>
              }
            />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  )
}
