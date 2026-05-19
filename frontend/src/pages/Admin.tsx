import { useEffect, useMemo, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  useReactTable,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  createColumnHelper,
  flexRender,
  type SortingState,
} from "@tanstack/react-table";
import {
  Settings,
  Users,
  Shield,
  ShieldOff,
  Trash2,
  ChevronUp,
  AlertCircle,
  CheckCircle2,
  HardDrive,
  ArrowUpDown,
  ArrowUp,
  ArrowDown,
  Search,
  ChevronLeft,
  ChevronRight,
  Zap,
} from "lucide-react";
import { adminApi, settingsApi, type AdminUser } from "../lib/api";
import { useAuthStore } from "../stores/auth";
import { useSettings } from "../hooks/useSettings";
import { Seo } from "../components/Seo";
import { Navigate } from "react-router-dom";

function fmtBytes(b: number) {
  if (b >= 1e9) return `${(b / 1e9).toFixed(1)} GB`;
  if (b >= 1e6) return `${(b / 1e6).toFixed(1)} MB`;
  if (b >= 1e3) return `${(b / 1e3).toFixed(1)} KB`;
  return `${b} B`;
}

// ── Settings panel ────────────────────────────────────────────────────────────

function SettingsPanel() {
  const settings = useSettings();
  const qc = useQueryClient();
  const [appName, setAppName] = useState(settings.app_name);
  const [logoUrl, setLogoUrl] = useState(settings.logo_url);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");

  const saveMutation = useMutation({
    mutationFn: (body: Parameters<typeof settingsApi.update>[0]) =>
      settingsApi.update(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["app-settings"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    },
    onError: (err: any) =>
      setError(err?.response?.data?.error ?? "Failed to save"),
  });

  const toggleSignup = useMutation({
    mutationFn: () =>
      settingsApi.update({ signup_enabled: !settings.signup_enabled }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["app-settings"] }),
    onError: (err: any) =>
      setError(err?.response?.data?.error ?? "Failed to update"),
  });

  return (
    <section className="bg-gray-900 border border-gray-800 rounded-xl p-6 space-y-5">
      <h2 className="text-sm font-semibold text-white flex items-center gap-2">
        <Settings className="w-4 h-4 text-violet-400" /> App settings
      </h2>

      {error && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
        </div>
      )}
      {saved && (
        <div className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-3 py-2.5 text-sm">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          Settings saved
        </div>
      )}

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div className="space-y-2">
          <label className="block text-xs font-medium text-gray-400">
            App name
          </label>
          <input
            value={appName}
            onChange={(e) => setAppName(e.target.value)}
            maxLength={64}
            className="w-full bg-gray-800 border border-gray-700 text-white rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
          />
        </div>
        <div className="space-y-2">
          <label className="block text-xs font-medium text-gray-400">
            Logo URL{" "}
            <span className="text-gray-600">(blank = default icon)</span>
          </label>
          <input
            value={logoUrl}
            onChange={(e) => setLogoUrl(e.target.value)}
            placeholder="https://example.com/logo.png"
            className="w-full bg-gray-800 border border-gray-700 text-white placeholder-gray-600 rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
          />
        </div>
      </div>

      <div className="flex items-center justify-between pt-1">
        <button
          onClick={() => {
            setError("");
            saveMutation.mutate({ app_name: appName, logo_url: logoUrl });
          }}
          disabled={saveMutation.isPending}
          className="bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        >
          {saveMutation.isPending ? "Saving…" : "Save changes"}
        </button>

        <div className="flex items-center gap-3">
          <div className="text-right">
            <p className="text-sm font-medium text-white">User registration</p>
            <p className="text-xs text-gray-500">
              {settings.signup_enabled
                ? "Open — anyone can sign up"
                : "Closed — invite-only"}
            </p>
          </div>
          <button
            onClick={() => {
              setError("");
              toggleSignup.mutate();
            }}
            disabled={toggleSignup.isPending}
            className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
              settings.signup_enabled ? "bg-violet-600" : "bg-gray-700"
            } disabled:opacity-50`}
          >
            <span
              className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
                settings.signup_enabled ? "translate-x-5" : "translate-x-0"
              }`}
            />
          </button>
        </div>
      </div>
    </section>
  );
}

function AuthProvidersPanel() {
  const qc = useQueryClient();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const { data } = useQuery({
    queryKey: ["admin-auth-providers"],
    queryFn: () => adminApi.getAuthProviders().then((r) => r.data),
  });
  const [form, setForm] = useState({
    local_enabled: true,
    github_enabled: false,
    github_client_id: "",
    github_client_secret: "",
    github_redirect_uri: "",
    gitlab_enabled: false,
    gitlab_base_url: "",
    gitlab_client_id: "",
    gitlab_client_secret: "",
    gitlab_redirect_uri: "",
  });

  useEffect(() => {
    if (!data) return;
    setForm({
      local_enabled: data.local_enabled,
      github_enabled: data.github.enabled,
      github_client_id: data.github.client_id ?? "",
      github_client_secret: "",
      github_redirect_uri: data.github.redirect_uri ?? "",
      gitlab_enabled: data.gitlab.enabled,
      gitlab_base_url: data.gitlab.base_url ?? "",
      gitlab_client_id: data.gitlab.client_id ?? "",
      gitlab_client_secret: "",
      gitlab_redirect_uri: data.gitlab.redirect_uri ?? "",
    });
  }, [data]);

  const saveMutation = useMutation({
    mutationFn: (body: Parameters<typeof adminApi.updateAuthProviders>[0]) =>
      adminApi.updateAuthProviders(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["admin-auth-providers"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    },
    onError: (err: any) =>
      setError(err?.response?.data?.error ?? "Failed to save"),
  });

  return (
    <section className="bg-gray-900 border border-gray-800 rounded-xl p-6 space-y-5">
      <h2 className="text-sm font-semibold text-white flex items-center gap-2">
        <Shield className="w-4 h-4 text-violet-400" /> Auth providers
      </h2>

      {error && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
        </div>
      )}
      {saved && (
        <div className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-3 py-2.5 text-sm">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          Auth provider settings saved
        </div>
      )}

      <div className="grid gap-6">
        <ProviderCard
          title="Local auth"
          enabled={form.local_enabled}
          onToggle={(enabled) => setForm((f) => ({ ...f, local_enabled: enabled }))}
        />
        <ProviderForm
          title="GitHub"
          enabled={form.github_enabled}
          onToggle={(enabled) => setForm((f) => ({ ...f, github_enabled: enabled }))}
          fields={[
            ["Client ID", form.github_client_id, "github_client_id"],
            ["Client Secret", form.github_client_secret, "github_client_secret", "password"],
            ["Redirect URI", form.github_redirect_uri, "github_redirect_uri"],
          ]}
          onFieldChange={(key, value) =>
            setForm((f) => ({ ...f, [key]: value }))
          }
        />
        <ProviderForm
          title="GitLab"
          enabled={form.gitlab_enabled}
          onToggle={(enabled) => setForm((f) => ({ ...f, gitlab_enabled: enabled }))}
          fields={[
            ["Base URL", form.gitlab_base_url, "gitlab_base_url"],
            ["Client ID", form.gitlab_client_id, "gitlab_client_id"],
            ["Client Secret", form.gitlab_client_secret, "gitlab_client_secret", "password"],
            ["Redirect URI", form.gitlab_redirect_uri, "gitlab_redirect_uri"],
          ]}
          onFieldChange={(key, value) =>
            setForm((f) => ({ ...f, [key]: value }))
          }
        />
      </div>

      <div className="flex justify-end">
        <button
          onClick={() => {
            setError("");
            saveMutation.mutate({
              ...form,
              github_client_secret: form.github_client_secret || undefined,
              gitlab_client_secret: form.gitlab_client_secret || undefined,
            });
          }}
          disabled={saveMutation.isPending}
          className="bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        >
          {saveMutation.isPending ? "Saving…" : "Save provider settings"}
        </button>
      </div>
    </section>
  );
}

function ProviderCard({
  title,
  enabled,
  onToggle,
}: {
  title: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-950/40 p-4 flex items-center justify-between gap-4">
      <div>
        <p className="text-sm font-medium text-white">{title}</p>
        <p className="text-xs text-gray-500">
          {enabled ? "Enabled" : "Disabled"}
        </p>
      </div>
      <button
        onClick={() => onToggle(!enabled)}
        className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
          enabled ? "bg-violet-600" : "bg-gray-700"
        }`}
      >
        <span
          className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
            enabled ? "translate-x-5" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}

function ProviderForm({
  title,
  enabled,
  onToggle,
  fields,
  onFieldChange,
}: {
  title: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  fields: Array<[
    label: string,
    value: string,
    key:
      | "github_client_id"
      | "github_client_secret"
      | "github_redirect_uri"
      | "gitlab_base_url"
      | "gitlab_client_id"
      | "gitlab_client_secret"
      | "gitlab_redirect_uri",
    type?: string,
  ]>;
  onFieldChange: (
    key:
      | "github_client_id"
      | "github_client_secret"
      | "github_redirect_uri"
      | "gitlab_base_url"
      | "gitlab_client_id"
      | "gitlab_client_secret"
      | "gitlab_redirect_uri",
    value: string,
  ) => void;
}) {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-950/40 p-4 space-y-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="text-sm font-medium text-white">{title}</p>
          <p className="text-xs text-gray-500">
            {enabled ? "Enabled" : "Disabled"}
          </p>
        </div>
        <button
          onClick={() => onToggle(!enabled)}
          className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
            enabled ? "bg-violet-600" : "bg-gray-700"
          }`}
        >
          <span
            className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
              enabled ? "translate-x-5" : "translate-x-0"
            }`}
          />
        </button>
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        {fields.map(([label, value, key, type]) => (
          <div key={key} className="space-y-2">
            <label className="block text-xs font-medium text-gray-400">
              {label}
            </label>
            <input
              value={value}
              type={type ?? "text"}
              onChange={(e) => onFieldChange(key, e.target.value)}
              className="w-full bg-gray-800 border border-gray-700 text-white rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
            />
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Users table ───────────────────────────────────────────────────────────────

const columnHelper = createColumnHelper<AdminUser>();

function SortIcon({ sorted }: { sorted: false | "asc" | "desc" }) {
  if (!sorted)
    return <ArrowUpDown className="w-3.5 h-3.5 text-gray-600 shrink-0" />;
  return sorted === "asc" ? (
    <ArrowUp className="w-3.5 h-3.5 text-violet-400 shrink-0" />
  ) : (
    <ArrowDown className="w-3.5 h-3.5 text-violet-400 shrink-0" />
  );
}

function UsersPanel({ currentUserId }: { currentUserId: string }) {
  const qc = useQueryClient();
  const [globalFilter, setGlobalFilter] = useState("");
  const [sorting, setSorting] = useState<SortingState>([]);
  const [actionError, setActionError] = useState("");

  const { data: users = [], isLoading } = useQuery({
    queryKey: ["admin-users"],
    queryFn: () => adminApi.listUsers().then((r) => r.data),
  });

  const mutate = (fn: () => Promise<unknown>) => {
    setActionError("");
    fn()
      .then(() => qc.invalidateQueries({ queryKey: ["admin-users"] }))
      .catch((err: any) =>
        setActionError(err?.response?.data?.error ?? "Action failed"),
      );
  };

  const columns = useMemo(
    () => [
      columnHelper.accessor("username", {
        header: "Username",
        cell: (info) => {
          const u = info.row.original;
          return (
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-white">
                {u.username}
              </span>
              {u.id === currentUserId && (
                <span className="text-xs text-gray-600">(you)</span>
              )}
              {u.is_admin !== 0 && (
                <span className="text-xs bg-violet-950/60 text-violet-400 border border-violet-800/40 px-1.5 py-0.5 rounded">
                  admin
                </span>
              )}
              {u.banned_at && (
                <span className="text-xs bg-red-950/60 text-red-400 border border-red-800/40 px-1.5 py-0.5 rounded">
                  banned
                </span>
              )}
            </div>
          );
        },
      }),
      columnHelper.accessor("email", {
        header: "Email",
        cell: (info) => (
          <span className="text-sm text-gray-400">{info.getValue()}</span>
        ),
      }),
      columnHelper.accessor("storage_usage_bytes", {
        header: "Storage",
        cell: (info) => {
          const u = info.row.original;
          return (
            <span className="flex items-center gap-1 text-sm text-gray-400">
              <HardDrive className="w-3.5 h-3.5 text-gray-600 shrink-0" />
              {fmtBytes(info.getValue())}
              {u.storage_quota_bytes != null && (
                <span className="text-gray-600">
                  / {fmtBytes(u.storage_quota_bytes)}
                </span>
              )}
            </span>
          );
        },
      }),
      columnHelper.accessor("created_at", {
        header: "Joined",
        cell: (info) => (
          <span className="text-sm text-gray-500">
            {new Date(info.getValue()).toLocaleDateString()}
          </span>
        ),
      }),
      columnHelper.display({
        id: "actions",
        header: "",
        cell: (info) => {
          const u = info.row.original;
          if (u.id === currentUserId) return null;
          return (
            <div className="flex items-center gap-1 justify-end">
              {u.is_admin === 0 && (
                <ActionButton
                  icon={<ChevronUp className="w-3.5 h-3.5" />}
                  label="Promote"
                  onClick={() => mutate(() => adminApi.promoteUser(u.id))}
                  color="violet"
                />
              )}
              <ActionButton
                icon={
                  u.banned_at ? (
                    <Shield className="w-3.5 h-3.5" />
                  ) : (
                    <ShieldOff className="w-3.5 h-3.5" />
                  )
                }
                label={u.banned_at ? "Unban" : "Ban"}
                onClick={() => mutate(() => adminApi.banUser(u.id))}
                color="amber"
              />
              <ActionButton
                icon={<Trash2 className="w-3.5 h-3.5" />}
                label="Purge"
                onClick={() => {
                  if (
                    confirm(
                      `Permanently delete '${u.username}' and all their data?`,
                    )
                  ) {
                    mutate(() => adminApi.purgeUser(u.id));
                  }
                }}
                color="red"
              />
            </div>
          );
        },
      }),
    ],
    [currentUserId],
  );

  const table = useReactTable({
    data: users,
    columns,
    state: { globalFilter, sorting },
    onGlobalFilterChange: setGlobalFilter,
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageSize: 20 } },
  });

  return (
    <section className="bg-gray-900 border border-gray-800 rounded-xl overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-gray-800 flex items-center justify-between gap-4">
        <h2 className="text-sm font-semibold text-white flex items-center gap-2 shrink-0">
          <Users className="w-4 h-4 text-violet-400" />
          Users
          <span className="text-xs font-normal text-gray-500 ml-1">
            {table.getFilteredRowModel().rows.length} / {users.length}
          </span>
        </h2>
        <div className="relative max-w-xs w-full">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" />
          <input
            value={globalFilter}
            onChange={(e) => setGlobalFilter(e.target.value)}
            placeholder="Search users…"
            className="w-full bg-gray-800 border border-gray-700 text-white placeholder-gray-600 rounded-lg pl-8 pr-3 py-2 text-sm focus:outline-none focus:border-violet-600 transition-colors"
          />
        </div>
      </div>

      {actionError && (
        <div className="px-5 py-3 flex items-center gap-2 text-red-400 bg-red-950/30 border-b border-red-900/40 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {actionError}
        </div>
      )}

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-left">
          <thead>
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id} className="border-b border-gray-800">
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    className="px-5 py-3 text-xs font-medium text-gray-500 whitespace-nowrap"
                  >
                    {header.isPlaceholder ? null : header.column.getCanSort() ? (
                      <button
                        onClick={header.column.getToggleSortingHandler()}
                        className="flex items-center gap-1.5 hover:text-gray-300 transition-colors"
                      >
                        {flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                        <SortIcon sorted={header.column.getIsSorted()} />
                      </button>
                    ) : (
                      flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )
                    )}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {isLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b border-gray-800/50">
                  {columns.map((_, j) => (
                    <td key={j} className="px-5 py-3.5">
                      <div className="h-4 rounded bg-gray-800 animate-pulse" />
                    </td>
                  ))}
                </tr>
              ))
            ) : table.getRowModel().rows.length === 0 ? (
              <tr>
                <td
                  colSpan={columns.length}
                  className="px-5 py-12 text-center text-sm text-gray-600"
                >
                  {globalFilter
                    ? `No users matching "${globalFilter}"`
                    : "No users found"}
                </td>
              </tr>
            ) : (
              table.getRowModel().rows.map((row) => (
                <tr
                  key={row.id}
                  className="border-b border-gray-800/50 hover:bg-gray-800/30 transition-colors"
                >
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="px-5 py-3.5">
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {table.getPageCount() > 1 && (
        <div className="px-5 py-3 border-t border-gray-800 flex items-center justify-between text-sm">
          <span className="text-gray-500">
            Page {table.getState().pagination.pageIndex + 1} of{" "}
            {table.getPageCount()}
          </span>
          <div className="flex items-center gap-1">
            <PaginationButton
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
              icon={<ChevronLeft className="w-4 h-4" />}
            />
            <PaginationButton
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
              icon={<ChevronRight className="w-4 h-4" />}
            />
          </div>
        </div>
      )}
    </section>
  );
}

function ActionButton({
  icon,
  label,
  onClick,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  color: "violet" | "amber" | "red";
}) {
  const colors = {
    violet: "text-violet-400 hover:bg-violet-950/50 hover:text-violet-300",
    amber: "text-amber-400 hover:bg-amber-950/50 hover:text-amber-300",
    red: "text-red-400 hover:bg-red-950/50 hover:text-red-300",
  };
  return (
    <button
      onClick={onClick}
      title={label}
      className={`flex items-center gap-1 px-2 py-1.5 rounded text-xs transition-colors ${colors[color]}`}
    >
      {icon}
      {label}
    </button>
  );
}

function PaginationButton({
  onClick,
  disabled,
  icon,
}: {
  onClick: () => void;
  disabled: boolean;
  icon: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="p-1.5 rounded text-gray-400 hover:text-white hover:bg-gray-800 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
    >
      {icon}
    </button>
  );
}

// ── Observability Panel ────────────────────────────────────────────────────────

function ObservabilityPanel() {
  const qc = useQueryClient();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const [testingGrafana, setTestingGrafana] = useState(false);
  const { data } = useQuery({
    queryKey: ["admin-observability"],
    queryFn: () => adminApi.getObservability().then((r) => r.data),
  });
  const [form, setForm] = useState({
    otel_endpoint: "",
    grafana_url: "",
    grafana_api_token: "",
  });

  useEffect(() => {
    if (!data) return;
    setForm({
      otel_endpoint: data.otel_endpoint ?? "",
      grafana_url: data.grafana_url ?? "",
      grafana_api_token: "",
    });
  }, [data]);

  const saveMutation = useMutation({
    mutationFn: (body: Parameters<typeof adminApi.updateObservability>[0]) =>
      adminApi.updateObservability(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["admin-observability"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    },
    onError: (err: any) =>
      setError(err?.response?.data?.error ?? "Failed to save"),
  });

  return (
    <section className="bg-gray-900 border border-gray-800 rounded-xl p-6 space-y-5">
      <h2 className="text-sm font-semibold text-white flex items-center gap-2">
        <Zap className="w-4 h-4 text-violet-400" /> Observability
      </h2>

      {error && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2.5 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
        </div>
      )}
      {saved && (
        <div className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-3 py-2.5 text-sm">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          Observability settings saved
        </div>
      )}

      <div className="space-y-4">
        <div className="space-y-2">
          <label className="block text-xs font-medium text-gray-400">
            OpenTelemetry Endpoint (optional)
          </label>
          <p className="text-xs text-gray-500">
            OTLP HTTP endpoint for telemetry collection (e.g., http://localhost:4318)
          </p>
          <input
            value={form.otel_endpoint}
            onChange={(e) => setForm((f) => ({ ...f, otel_endpoint: e.target.value }))}
            placeholder="http://otel-collector:4318"
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500"
          />
        </div>

        <div className="space-y-2">
          <label className="block text-xs font-medium text-gray-400">
            Grafana URL (optional)
          </label>
          <p className="text-xs text-gray-500">
            Grafana instance URL for dashboard integration (e.g., https://grafana.example.com)
          </p>
          <input
            value={form.grafana_url}
            onChange={(e) => setForm((f) => ({ ...f, grafana_url: e.target.value }))}
            placeholder="https://grafana.example.com"
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500"
          />
        </div>

        <div className="space-y-2">
          <label className="block text-xs font-medium text-gray-400">
            Grafana API Token (optional)
          </label>
          <p className="text-xs text-gray-500">
            API token for authenticating with Grafana
          </p>
          <input
            type="password"
            value={form.grafana_api_token}
            onChange={(e) => setForm((f) => ({ ...f, grafana_api_token: e.target.value }))}
            placeholder="••••••••••••••••"
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500"
          />
          {data?.grafana_configured && (
            <p className="text-xs text-emerald-400">✓ Grafana is configured</p>
          )}
        </div>
      </div>

      <div className="flex justify-end gap-3">
        {data?.grafana_configured && form.grafana_url && (
          <button
            onClick={async () => {
              setTestingGrafana(true);
              setError("");
              try {
                const response = await fetch(
                  `${form.grafana_url}/api/health`,
                  {
                    headers: {
                      Authorization: `Bearer ${form.grafana_api_token || "test"}`,
                    },
                  }
                ).then((r) => r.json());
                if (response.status === "ok") {
                  setSaved(true);
                  setTimeout(() => setSaved(false), 2500);
                } else {
                  setError("Grafana health check failed");
                }
              } catch (err) {
                setError("Could not reach Grafana. Check URL and token.");
              } finally {
                setTestingGrafana(false);
              }
            }}
            disabled={testingGrafana || saveMutation.isPending}
            className="text-gray-400 hover:text-white px-3 py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            {testingGrafana ? "Testing…" : "Test Grafana"}
          </button>
        )}
        <button
          onClick={() => {
            setError("");
            saveMutation.mutate({
              otel_endpoint: form.otel_endpoint || undefined,
              grafana_url: form.grafana_url || undefined,
              grafana_api_token: form.grafana_api_token || undefined,
            });
          }}
          disabled={saveMutation.isPending}
          className="bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        >
          {saveMutation.isPending ? "Saving…" : "Save observability settings"}
        </button>
      </div>
    </section>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────────

export function Admin() {
  const { user } = useAuthStore();
  const { app_name } = useSettings();
  const [tab, setTab] = useState<"general" | "auth" | "observability" | "users">("general");

  if (!user || user.is_admin === 0) {
    return <Navigate to="/" replace />;
  }

  return (
    <div className="max-w-5xl mx-auto px-6 py-8 space-y-6">
      <Seo
        title={`Admin | ${app_name}`}
        description="Manage users, moderation, and global application settings."
        canonical={`${window.location.origin}/admin`}
        robots="noindex,follow"
      />
      <h1 className="text-xl font-bold text-white">Admin panel</h1>
      <div className="flex gap-2">
        <TabButton active={tab === "general"} onClick={() => setTab("general")}>
          General
        </TabButton>
        <TabButton active={tab === "auth"} onClick={() => setTab("auth")}>
          Auth providers
        </TabButton>
        <TabButton active={tab === "observability"} onClick={() => setTab("observability")}>
          Observability
        </TabButton>
        <TabButton active={tab === "users"} onClick={() => setTab("users")}>
          Users
        </TabButton>
      </div>

      {tab === "general" && <SettingsPanel />}
      {tab === "auth" && <AuthProvidersPanel />}
      {tab === "observability" && <ObservabilityPanel />}
      {tab === "users" && <UsersPanel currentUserId={user.id} />}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-3 py-2 rounded-lg text-sm border transition-colors ${
        active
          ? "bg-violet-950/60 text-violet-300 border-violet-800/50"
          : "bg-gray-900 text-gray-400 border-gray-800 hover:text-white hover:bg-gray-800"
      }`}
    >
      {children}
    </button>
  );
}
