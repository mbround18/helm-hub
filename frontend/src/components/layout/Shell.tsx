import { Link, NavLink, useNavigate } from 'react-router-dom'
import { Package, LogOut, User, LayoutDashboard, Search } from 'lucide-react'
import { useAuthStore } from '../../stores/auth'
import clsx from 'clsx'

interface ShellProps {
  children: React.ReactNode
}

export function Shell({ children }: ShellProps) {
  const { user, clearAuth, isAuthenticated } = useAuthStore()
  const navigate = useNavigate()

  const handleLogout = () => {
    clearAuth()
    navigate('/login')
  }

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex">
      {/* Sidebar */}
      <aside className="w-60 shrink-0 bg-gray-900 border-r border-gray-800 flex flex-col">
        <div className="p-5 border-b border-gray-800">
          <Link to="/" className="flex items-center gap-2 text-violet-400 font-semibold text-lg">
            <Package className="w-5 h-5" />
            Helm Hub
          </Link>
        </div>

        <nav className="flex-1 p-3 space-y-1">
          <SidebarLink to="/" icon={<Search className="w-4 h-4" />} label="Explore" end />
          {isAuthenticated() && (
            <SidebarLink
              to={`/u/${user?.username}`}
              icon={<LayoutDashboard className="w-4 h-4" />}
              label="My Charts"
            />
          )}
        </nav>

        <div className="p-3 border-t border-gray-800">
          {isAuthenticated() ? (
            <div className="space-y-1">
              <SidebarLink
                to="/profile"
                icon={<User className="w-4 h-4" />}
                label={user?.username ?? ''}
              />
              <button
                onClick={handleLogout}
                className="flex items-center gap-2 w-full px-3 py-2 text-sm text-gray-400 hover:text-red-400 hover:bg-gray-800 rounded-md transition-colors"
              >
                <LogOut className="w-4 h-4" />
                Sign out
              </button>
            </div>
          ) : (
            <SidebarLink to="/login" icon={<User className="w-4 h-4" />} label="Sign in" />
          )}
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto">{children}</main>
    </div>
  )
}

function SidebarLink({
  to,
  icon,
  label,
  end,
}: {
  to: string
  icon: React.ReactNode
  label: string
  end?: boolean
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        clsx(
          'flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors',
          isActive
            ? 'bg-violet-950/60 text-violet-300 border border-violet-800/50'
            : 'text-gray-300 hover:text-white hover:bg-gray-800',
        )
      }
    >
      {icon}
      {label}
    </NavLink>
  )
}
