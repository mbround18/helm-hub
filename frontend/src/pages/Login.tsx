import { useState } from "react";
import { useNavigate, Link } from "react-router-dom";
import { authApi } from "../lib/api";
import { useAuthStore } from "../stores/auth";
import { AlertCircle, GitBranch } from "lucide-react";
import { AppLogo } from "../components/AppLogo";
import { useSettings } from "../hooks/useSettings";
import { Seo } from "../components/Seo";

export function Login() {
  const [form, setForm] = useState({
    username: "",
    password: "",
    totp_code: "",
  });
  const [needsTotp, setNeedsTotp] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const { setAuth } = useAuthStore();
  const navigate = useNavigate();
  const {
    signup_enabled,
    app_name,
    local_auth_enabled,
    github_auth_enabled,
    gitlab_auth_enabled,
  } = useSettings();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      const { data } = await authApi.login(
        form.username,
        form.password,
        needsTotp ? form.totp_code : undefined,
      );
      setAuth(data.token, data.user);
      navigate(`/u/${data.user.username}`);
    } catch (err: any) {
      const msg: string = err?.response?.data?.error ?? "Login failed";
      if (msg.toLowerCase().includes("totp")) setNeedsTotp(true);
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <Seo
        title={`Sign in | ${app_name}`}
        description="Sign in to manage Helm charts, uploads, and tokens."
        canonical={`${window.location.origin}/login`}
        robots="noindex,follow"
      />
      <div className="w-full max-w-sm">
        <div className="flex items-center justify-center text-violet-400 mb-8">
          <AppLogo size="md" />
        </div>

        <div className="bg-gray-900 border border-gray-800 rounded-xl p-6">
          <h1 className="text-lg font-semibold text-white mb-5">Sign in</h1>

          {error && (
            <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm mb-4">
              <AlertCircle className="w-4 h-4 shrink-0" />
              {error}
            </div>
          )}

          <div className="space-y-4">
            {local_auth_enabled ? (
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
                  {loading ? "Signing in…" : "Sign in"}
                </button>
              </form>
            ) : (
              <div className="rounded-lg border border-gray-800 bg-gray-950/60 p-4 text-sm text-gray-400">
                Local sign-in is disabled for this instance.
              </div>
            )}

            {(github_auth_enabled || gitlab_auth_enabled) && (
              <div className="space-y-3">
                <div className="text-xs uppercase tracking-wide text-gray-500">
                  Or continue with
                </div>
                {github_auth_enabled && (
                  <OAuthButton
                    icon={<GitBranch className="w-4 h-4" />}
                    label="GitHub"
                    onClick={() =>
                      window.location.assign(
                        authApi.githubSsoLogin("/profile"),
                      )
                    }
                  />
                )}
                {gitlab_auth_enabled && (
                  <OAuthButton
                    icon={<GitBranch className="w-4 h-4" />}
                    label="GitLab"
                    onClick={() =>
                      window.location.assign(
                        authApi.gitlabSsoLogin("/profile"),
                      )
                    }
                  />
                )}
              </div>
            )}
          </div>

          {signup_enabled && local_auth_enabled && (
            <p className="text-center text-sm text-gray-500 mt-5">
              No account?{" "}
              <Link
                to="/register"
                className="text-violet-400 hover:text-violet-300"
              >
                Register
              </Link>
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  type,
  value,
  onChange,
  placeholder,
  autoComplete,
}: {
  label: string;
  type: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  autoComplete?: string;
}) {
  return (
    <div>
      <label className="block text-xs font-medium text-gray-400 mb-1.5">
        {label}
      </label>
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
  );
}

function OAuthButton({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full flex items-center justify-center gap-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-white py-2.5 rounded-lg text-sm font-medium transition-colors"
    >
      {icon}
      Continue with {label}
    </button>
  );
}
