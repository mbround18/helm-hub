import { Suspense, lazy, useEffect } from "react";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  Outlet,
} from "react-router-dom";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { useAuthStore } from "./stores/auth";
import { authApi } from "./lib/api";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, staleTime: 30_000 } },
});

const Shell = lazy(async () => ({
  default: (await import("./components/layout/Shell")).Shell,
}));
const Explore = lazy(async () => ({
  default: (await import("./pages/Explore")).Explore,
}));
const Dashboard = lazy(async () => ({
  default: (await import("./pages/Dashboard")).Dashboard,
}));
const Profile = lazy(async () => ({
  default: (await import("./pages/Profile")).Profile,
}));
const ChartPage = lazy(async () => ({
  default: (await import("./pages/ChartPage")).ChartPage,
}));
const Admin = lazy(async () => ({
  default: (await import("./pages/Admin")).Admin,
}));
const Login = lazy(async () => ({
  default: (await import("./pages/Login")).Login,
}));
const AuthCallback = lazy(async () => ({
  default: (await import("./pages/AuthCallback")).AuthCallback,
}));
const Register = lazy(async () => ({
  default: (await import("./pages/Register")).Register,
}));

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuthStore();
  return isAuthenticated() ? <>{children}</> : <Navigate to="/login" replace />;
}

function AdminRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, clearAuth, setUser } = useAuthStore();
  const authed = isAuthenticated();
  const {
    data: me,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["auth-me"],
    queryFn: () => authApi.me().then((r) => r.data),
    enabled: authed,
    staleTime: 30_000,
    retry: false,
    refetchOnWindowFocus: false,
  });

  const status = (error as any)?.response?.status;
  const unauthorized = status === 401 || status === 404;

  useEffect(() => {
    if (me) setUser(me);
  }, [me, setUser]);

  useEffect(() => {
    if (unauthorized) clearAuth();
  }, [unauthorized, clearAuth]);

  if (!authed) return <Navigate to="/login" replace />;
  if (unauthorized) return <Navigate to="/login" replace />;
  if (isLoading) return <PageFallback />;
  if (!me) return <Navigate to="/" replace />;

  const role = me.role || (me.is_admin ? "Admin" : "User");
  const isAdminOrOwner = role === "Owner" || role === "Admin";
  return isAdminOrOwner ? <>{children}</> : <Navigate to="/" replace />;
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
  );
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
  );
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
  );
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
  );
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
            path="/auth/callback"
            element={
              <Suspense fallback={<AuthPageFallback />}>
                <AuthCallback />
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
          <Route
            path="/charts/:owner/:name"
            element={
              <Suspense fallback={<PageFallback />}>
                <ChartPage />
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
            <Route
              path="/admin"
              element={
                <AdminRoute>
                  <Admin />
                </AdminRoute>
              }
            />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
