import { useEffect, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import {
  Key,
  Plus,
  Trash2,
  Copy,
  Check,
  AlertCircle,
  Clock,
  Shield,
  X,
  GitBranch,
  RefreshCw,
  Link,
  CheckCircle2,
  Loader2,
  Package,
} from "lucide-react";
import {
  tokensApi,
  githubApi,
  gitlabApi,
  chartsApi,
  type ApiToken,
  type CreatedToken,
  type TokenTtlDays,
  type GithubRepo,
  type GitlabRepo,
  type ChartSyncEntry,
} from "../lib/api";
import { useAuthStore } from "../stores/auth";
import { useSettings } from "../hooks/useSettings";
import { usePermissions } from "../hooks/usePermissions";
import { Seo } from "../components/Seo";

const TTL_OPTIONS: { label: string; value: TokenTtlDays }[] = [
  { label: "30 days", value: 30 },
  { label: "60 days", value: 60 },
  { label: "90 days", value: 90 },
  { label: "180 days", value: 180 },
  { label: "1 year", value: 365 },
];

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function isExpired(iso: string) {
  return new Date(iso) < new Date();
}

function expiresLabel(iso: string) {
  if (isExpired(iso)) return "Expired";
  const days = Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000);
  if (days === 1) return "Expires tomorrow";
  if (days <= 14) return `Expires in ${days} days`;
  return `Expires ${formatDate(iso)}`;
}

// ── New-token modal ───────────────────────────────────────────────────────────

function NewTokenModal({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const [description, setDescription] = useState("");
  const [ttl, setTtl] = useState<TokenTtlDays>(90);
  const [created, setCreated] = useState<CreatedToken | null>(null);
  const [copied, setCopied] = useState(false);

  const create = useMutation({
    mutationFn: () => tokensApi.create(description.trim(), ttl),
    onSuccess: (res) => {
      setCreated(res.data);
      qc.invalidateQueries({ queryKey: ["tokens"] });
    },
  });

  const copyToken = () => {
    if (!created) return;
    navigator.clipboard.writeText(created.token);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-md rounded-xl border border-gray-800 bg-gray-900 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-800">
          <h2 className="text-base font-semibold text-white">
            {created ? "Token created" : "New access token"}
          </h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          {created ? (
            // ── Show token once ──────────────────────────────────────────────
            <>
              <div className="flex items-start gap-2 text-amber-400 bg-amber-950/40 border border-amber-800 rounded-lg px-4 py-3 text-sm">
                <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                <span>Copy this token now — it will not be shown again.</span>
              </div>

              <div className="relative group">
                <pre className="bg-gray-950 border border-gray-700 rounded-lg px-4 py-3 text-xs font-mono text-violet-300 break-all whitespace-pre-wrap">
                  {created.token}
                </pre>
                <button
                  onClick={copyToken}
                  className="absolute top-2 right-2 p-1.5 rounded bg-gray-800 text-gray-400 hover:text-white transition-colors"
                  title="Copy token"
                >
                  {copied ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                </button>
              </div>

              <p className="text-xs text-gray-500">
                Description:{" "}
                <span className="text-gray-300">{created.description}</span>
                {" · "}Expires{" "}
                <span className="text-gray-300">
                  {formatDate(created.expires_at)}
                </span>
              </p>

              <button
                onClick={onClose}
                className="w-full bg-violet-600 hover:bg-violet-500 text-white py-2 rounded-lg text-sm font-medium transition-colors"
              >
                Done
              </button>
            </>
          ) : (
            // ── Create form ──────────────────────────────────────────────────
            <>
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-gray-400">
                  Description
                </label>
                <input
                  type="text"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="e.g. CI/CD pipeline, local dev, GitHub Actions…"
                  maxLength={120}
                  className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-600 focus:outline-none focus:border-violet-500"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-gray-400">
                  Expiration
                </label>
                <div className="grid grid-cols-5 gap-1.5">
                  {TTL_OPTIONS.map((opt) => (
                    <button
                      key={opt.value}
                      onClick={() => setTtl(opt.value)}
                      className={
                        ttl === opt.value
                          ? "py-2 rounded-lg text-xs font-medium bg-violet-600 text-white"
                          : "py-2 rounded-lg text-xs font-medium bg-gray-800 text-gray-300 hover:bg-gray-700 transition-colors"
                      }
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>

              {create.isError && (
                <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-3 py-2 text-xs">
                  <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                  {(create.error as any)?.response?.data?.error ??
                    "Failed to create token"}
                </div>
              )}

              <button
                onClick={() => create.mutate()}
                disabled={create.isPending || description.trim().length === 0}
                className="w-full bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white py-2 rounded-lg text-sm font-medium transition-colors"
              >
                {create.isPending ? "Generating…" : "Generate token"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Token row ─────────────────────────────────────────────────────────────────

function TokenRow({
  token,
  onRevoke,
}: {
  token: ApiToken;
  onRevoke: () => void;
}) {
  const expired = isExpired(token.expires_at);
  return (
    <div className="flex items-start justify-between px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg gap-4">
      <div className="min-w-0 space-y-0.5">
        <div className="flex items-center gap-2">
          <Key className="w-3.5 h-3.5 text-violet-400 shrink-0" />
          <span className="text-sm font-medium text-white truncate">
            {token.description}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-gray-500">
          <span className="flex items-center gap-1">
            <Clock className="w-3 h-3" />
            <span className={expired ? "text-red-400" : ""}>
              {expiresLabel(token.expires_at)}
            </span>
          </span>
          <span>Created {formatDate(token.created_at)}</span>
          {token.last_used_at && (
            <span>Last used {formatDate(token.last_used_at)}</span>
          )}
        </div>
      </div>
      <button
        onClick={onRevoke}
        className="shrink-0 p-1.5 text-gray-500 hover:text-red-400 hover:bg-gray-800 rounded transition-colors"
        title="Revoke token"
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
}

// ── GitHub: add-repo modal ────────────────────────────────────────────────────

function AddRepoModal({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const [slug, setSlug] = useState("");
  const add = useMutation({
    mutationFn: () => githubApi.addRepo(slug.trim()),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["github-repos"] });
      onClose();
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-sm rounded-xl border border-gray-800 bg-gray-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-800">
          <h2 className="text-base font-semibold text-white">
            Link GitHub repository
          </h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="p-5 space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-gray-400">
              Repository
            </label>
            <input
              type="text"
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder="owner/repo-name"
              className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-600 focus:outline-none focus:border-violet-500 font-mono"
            />
            <p className="text-xs text-gray-600">
              Helm chart releases follow the{" "}
              <code className="text-gray-400">chart-name-1.2.3.tgz</code> naming
              convention.
            </p>
          </div>
          {add.isError && (
            <div className="flex items-center gap-2 text-red-400 text-xs bg-red-950/40 border border-red-800 rounded-lg px-3 py-2">
              <AlertCircle className="w-3.5 h-3.5 shrink-0" />
              {(add.error as any)?.response?.data?.error ??
                "Failed to add repository"}
            </div>
          )}
          <button
            onClick={() => add.mutate()}
            disabled={add.isPending || !slug.includes("/")}
            className="w-full bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white py-2 rounded-lg text-sm font-medium transition-colors"
          >
            {add.isPending ? "Adding…" : "Add repository"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── GitHub: sync result display ───────────────────────────────────────────────

function SyncResults({
  entries,
  repo,
}: {
  entries: ChartSyncEntry[];
  repo: string;
}) {
  const imported = entries.filter((e) => e.status === "imported");
  const skipped = entries.filter((e) => e.status === "skipped");
  const failed = entries.filter((e) => e.status === "failed");

  return (
    <div className="mt-3 space-y-2 text-xs">
      {imported.length > 0 && (
        <div className="bg-emerald-950/40 border border-emerald-800 rounded-lg px-3 py-2">
          <p className="text-emerald-400 font-medium mb-1">
            {imported.length} version{imported.length !== 1 ? "s" : ""} imported
            from {repo}
          </p>
          {imported.map((e) => (
            <p key={e.chart + e.version} className="text-emerald-600 font-mono">
              {e.chart} v{e.version}
            </p>
          ))}
        </div>
      )}
      {failed.length > 0 && (
        <div className="bg-red-950/40 border border-red-800 rounded-lg px-3 py-2">
          <p className="text-red-400 font-medium mb-1">
            {failed.length} failed
          </p>
          {failed.map((e) => (
            <p key={e.chart + e.version} className="text-red-600">
              {e.chart} v{e.version}: {e.message}
            </p>
          ))}
        </div>
      )}
      {imported.length === 0 && failed.length === 0 && (
        <p className="text-gray-600">
          All {skipped.length} version{skipped.length !== 1 ? "s" : ""} already
          imported — nothing new.
        </p>
      )}
    </div>
  );
}

// ── GitHub: repo row ──────────────────────────────────────────────────────────

function RepoRow({ repo }: { repo: GithubRepo }) {
  const qc = useQueryClient();
  const [syncResult, setSyncResult] = useState<ChartSyncEntry[] | null>(null);

  const sync = useMutation({
    mutationFn: () => githubApi.syncRepo(repo.id),
    onSuccess: (res) => {
      setSyncResult(res.data.entries);
      qc.invalidateQueries({ queryKey: ["github-repos"] });
      qc.invalidateQueries({ queryKey: ["charts"] });
    },
  });

  const remove = useMutation({
    mutationFn: () => githubApi.removeRepo(repo.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["github-repos"] }),
  });

  const fullName = `${repo.repo_owner}/${repo.repo_name}`;

  return (
    <div className="px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg space-y-2">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Link className="w-3.5 h-3.5 text-violet-400 shrink-0" />
            <a
              href={`https://github.com/${fullName}`}
              target="_blank"
              rel="noreferrer"
              className="text-sm font-mono text-white hover:text-violet-400 transition-colors truncate"
            >
              {fullName}
            </a>
          </div>
          {repo.last_synced_at && (
            <p className="text-xs text-gray-600 mt-0.5 pl-5">
              Last synced {new Date(repo.last_synced_at).toLocaleString()}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => sync.mutate()}
            disabled={sync.isPending}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-gray-800 hover:bg-gray-700 text-gray-300 hover:text-white rounded-lg transition-colors disabled:opacity-50"
            title="Sync releases from GitHub"
          >
            {sync.isPending ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <RefreshCw className="w-3.5 h-3.5" />
            )}
            {sync.isPending ? "Syncing…" : "Sync now"}
          </button>
          <button
            onClick={() => remove.mutate()}
            disabled={remove.isPending}
            className="p-1.5 text-gray-500 hover:text-red-400 hover:bg-gray-800 rounded transition-colors"
            title="Unlink repository"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      {sync.isError && (
        <p className="text-xs text-red-400">
          {(sync.error as any)?.response?.data?.error ?? "Sync failed"}
        </p>
      )}
      {syncResult && <SyncResults entries={syncResult} repo={fullName} />}
    </div>
  );
}

// ── GitHub section ────────────────────────────────────────────────────────────

function GithubSection() {
  const settings = useSettings();
  const qc = useQueryClient();
  const [showAddRepo, setShowAddRepo] = useState(false);

  const { data: connData, isLoading: connLoading } = useQuery({
    queryKey: ["github-connection"],
    queryFn: () => githubApi.getConnection().then((r) => r.data.connection),
  });

  const { data: repos = [], isLoading: reposLoading } = useQuery({
    queryKey: ["github-repos"],
    queryFn: () => githubApi.listRepos().then((r) => r.data),
    enabled: !!connData,
  });

  const disconnect = useMutation({
    mutationFn: () => githubApi.deleteConnection(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["github-connection"] });
      qc.invalidateQueries({ queryKey: ["github-repos"] });
    },
  });

  const connectGitHub = async () => {
    try {
      const res = await githubApi.oauthUrl(window.location.href);
      window.location.href = res.data.url;
    } catch {
      // Server likely doesn't have GitHub configured
    }
  };

  // Don't show GitHub section if not configured by admin
  if (!settings.github_auth_enabled) {
    return null;
  }

  return (
    <div className="space-y-4">
      {showAddRepo && <AddRepoModal onClose={() => setShowAddRepo(false)} />}

      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-white flex items-center gap-2">
            <GitBranch className="w-4 h-4 text-violet-400" />
            GitHub
          </h2>
          <p className="text-xs text-gray-500 mt-0.5">
            Link your GitHub account for SSO and sync chart releases
            automatically.
          </p>
        </div>
        {connData && (
          <button
            onClick={() => setShowAddRepo(true)}
            className="flex items-center gap-1.5 bg-violet-600 hover:bg-violet-500 text-white px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            Add repo
          </button>
        )}
      </div>

      {connLoading ? (
        <div className="h-16 bg-gray-800 rounded-lg animate-pulse" />
      ) : connData ? (
        <>
          {/* Connected account card */}
          <div className="flex items-center justify-between px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg">
            <div className="flex items-center gap-3">
              {connData.avatar_url && (
                <img
                  src={connData.avatar_url}
                  alt={connData.github_username}
                  className="w-8 h-8 rounded-full"
                />
              )}
              <div>
                <div className="flex items-center gap-1.5">
                  <span className="text-sm font-medium text-white">
                    {connData.github_username}
                  </span>
                  <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                </div>
                <p className="text-xs text-gray-500">
                  GitHub account connected
                </p>
              </div>
            </div>
            <button
              onClick={() => disconnect.mutate()}
              disabled={disconnect.isPending}
              className="text-xs text-gray-500 hover:text-red-400 transition-colors"
            >
              Disconnect
            </button>
          </div>

          {/* Linked repos */}
          {reposLoading ? (
            <div className="h-14 bg-gray-800 rounded-lg animate-pulse" />
          ) : repos.length === 0 ? (
            <div className="text-center py-8 text-gray-600 border border-gray-800 rounded-lg">
              <Link className="w-6 h-6 mx-auto mb-2 opacity-40" />
              <p className="text-sm">No repositories linked yet.</p>
              <button
                onClick={() => setShowAddRepo(true)}
                className="mt-2 text-xs text-violet-400 hover:text-violet-300 transition-colors"
              >
                Link your first repo →
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {repos.map((r) => (
                <RepoRow key={r.id} repo={r} />
              ))}
            </div>
          )}
        </>
      ) : (
        <button
          onClick={connectGitHub}
          className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-gray-900 hover:bg-gray-800 border border-gray-800 hover:border-gray-700 text-gray-300 hover:text-white rounded-lg text-sm font-medium transition-colors"
        >
          <GitBranch className="w-4 h-4" />
          Connect GitHub account
        </button>
      )}
    </div>
  );
}

// ── GitLab: add-repo modal ────────────────────────────────────────────────────

function AddGitLabRepoModal({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const [slug, setSlug] = useState("");
  const add = useMutation({
    mutationFn: () => gitlabApi.addRepo(slug.trim()),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["gitlab-repos"] });
      onClose();
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-sm rounded-xl border border-gray-800 bg-gray-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-800">
          <h2 className="text-base font-semibold text-white">
            Link GitLab repository
          </h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="p-5 space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-gray-400">
              Repository
            </label>
            <input
              type="text"
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder="e.g. owner/repo"
              className="w-full px-3 py-2 bg-gray-800 text-white text-sm rounded-lg border border-gray-700 focus:border-violet-600 focus:outline-none"
            />
            <p className="text-xs text-gray-500">
              Format: <code className="text-violet-400">owner/repo</code>
            </p>
          </div>
          <button
            disabled={add.isPending || !slug.trim()}
            onClick={() => add.mutate()}
            className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-violet-600 hover:bg-violet-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg text-sm font-medium transition-colors"
          >
            {add.isPending ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Linking...
              </>
            ) : (
              <>
                <Link className="w-4 h-4" />
                Link repository
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── GitLab: sync result display ────────────────────────────────────────────────

function GitLabSyncResult({
  entries,
  onClose,
}: {
  entries: ChartSyncEntry[];
  onClose: () => void;
}) {
  const imported = entries.filter((e) => e.status === "imported").length;
  const skipped = entries.filter((e) => e.status === "skipped").length;
  const failed = entries.filter((e) => e.status === "failed").length;

  return (
    <div className="space-y-2 p-4 border border-gray-800 rounded-lg bg-gray-900">
      <div className="flex items-center justify-between">
        <div className="flex gap-4 text-xs">
          <span className="text-emerald-400">
            {imported} imported
          </span>
          <span className="text-yellow-400">
            {skipped} skipped
          </span>
          {failed > 0 && (
            <span className="text-red-400">
              {failed} failed
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="text-xs text-gray-500 hover:text-white transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {failed > 0 && (
        <div className="space-y-1 border-t border-gray-800 pt-2">
          {entries
            .filter((e) => e.status === "failed")
            .map((e, i) => (
              <div key={i} className="text-xs text-red-400">
                {e.chart} {e.version}: {e.message}
              </div>
            ))}
        </div>
      )}
    </div>
  );
}

// ── GitLab: repo row ───────────────────────────────────────────────────────────

function GitLabRepoRow({ repo }: { repo: GitlabRepo }) {
  const qc = useQueryClient();
  const [syncResult, setSyncResult] = useState<ChartSyncEntry[] | null>(null);

  const sync = useMutation({
    mutationFn: () => gitlabApi.syncRepo(repo.id),
    onSuccess: (res) => {
      setSyncResult(res.data.entries);
      qc.invalidateQueries({ queryKey: ["gitlab-repos"] });
      qc.invalidateQueries({ queryKey: ["charts"] });
    },
  });

  const remove = useMutation({
    mutationFn: () => gitlabApi.removeRepo(repo.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["gitlab-repos"] }),
  });

  const fullName = `${repo.repo_owner}/${repo.repo_name}`;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg">
        <a
          href={`https://gitlab.com/${fullName}`}
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 text-sm font-medium text-violet-400 hover:text-violet-300 transition-colors"
        >
          <GitBranch className="w-4 h-4" />
          {fullName}
        </a>
        <div className="flex items-center gap-2">
          <button
            onClick={() => sync.mutate()}
            disabled={sync.isPending}
            title="Sync releases from GitLab"
            className="text-gray-400 hover:text-violet-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <RefreshCw
              className={`w-4 h-4 ${
                sync.isPending ? "animate-spin" : ""
              }`}
            />
          </button>
          <button
            onClick={() => remove.mutate()}
            disabled={remove.isPending}
            className="text-gray-400 hover:text-red-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>
      {syncResult && (
        <GitLabSyncResult
          entries={syncResult}
          onClose={() => setSyncResult(null)}
        />
      )}
    </div>
  );
}

// ── GitLab section ─────────────────────────────────────────────────────────────

function GitLabSection() {
  const settings = useSettings();
  const qc = useQueryClient();
  const [showAddRepo, setShowAddRepo] = useState(false);

  const { data: connData, isLoading: connLoading } = useQuery({
    queryKey: ["gitlab-connection"],
    queryFn: () => gitlabApi.getConnection().then((r) => r.data.connection),
  });

  const { data: repos = [], isLoading: reposLoading } = useQuery({
    queryKey: ["gitlab-repos"],
    queryFn: () => gitlabApi.listRepos().then((r) => r.data),
    enabled: !!connData,
  });

  const disconnect = useMutation({
    mutationFn: () => gitlabApi.deleteConnection(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["gitlab-connection"] });
      qc.invalidateQueries({ queryKey: ["gitlab-repos"] });
    },
  });

  const connectGitLab = async () => {
    try {
      const res = await gitlabApi.oauthUrl(window.location.href);
      window.location.href = res.data.url;
    } catch {
      // Server likely doesn't have GitLab configured
    }
  };

  // Don't show GitLab section if not configured by admin
  if (!settings.gitlab_auth_enabled) {
    return null;
  }

  return (
    <div className="space-y-4">
      {showAddRepo && (
        <AddGitLabRepoModal onClose={() => setShowAddRepo(false)} />
      )}

      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-white flex items-center gap-2">
            <GitBranch className="w-4 h-4 text-violet-400" />
            GitLab
          </h2>
          <p className="text-xs text-gray-500 mt-0.5">
            Link your GitLab account for SSO and sync chart releases
            automatically.
          </p>
        </div>
        {connData && (
          <button
            onClick={() => setShowAddRepo(true)}
            className="flex items-center gap-1.5 bg-violet-600 hover:bg-violet-500 text-white px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            Add repo
          </button>
        )}
      </div>

      {connLoading ? (
        <div className="h-16 bg-gray-800 rounded-lg animate-pulse" />
      ) : connData ? (
        <>
          {/* Connected account card */}
          <div className="flex items-center justify-between px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg">
            <div className="flex items-center gap-3">
              {connData.avatar_url && (
                <img
                  src={connData.avatar_url}
                  alt={connData.gitlab_username}
                  className="w-8 h-8 rounded-full"
                />
              )}
              <div>
                <div className="flex items-center gap-1.5">
                  <span className="text-sm font-medium text-white">
                    {connData.gitlab_username}
                  </span>
                  <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                </div>
                <p className="text-xs text-gray-500">
                  GitLab account connected
                </p>
              </div>
            </div>
            <button
              onClick={() => disconnect.mutate()}
              disabled={disconnect.isPending}
              className="text-xs text-gray-500 hover:text-red-400 transition-colors"
            >
              Disconnect
            </button>
          </div>

          {/* Linked repos */}
          {reposLoading ? (
            <div className="h-14 bg-gray-800 rounded-lg animate-pulse" />
          ) : repos.length === 0 ? (
            <div className="text-center py-8 text-gray-600 border border-gray-800 rounded-lg">
              <Link className="w-6 h-6 mx-auto mb-2 opacity-40" />
              <p className="text-sm">No repositories linked yet.</p>
              <button
                onClick={() => setShowAddRepo(true)}
                className="mt-2 text-xs text-violet-400 hover:text-violet-300 transition-colors"
              >
                Link your first repo →
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {repos.map((r) => (
                <GitLabRepoRow key={r.id} repo={r} />
              ))}
            </div>
          )}
        </>
      ) : (
        <button
          onClick={connectGitLab}
          className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-gray-900 hover:bg-gray-800 border border-gray-800 hover:border-gray-700 text-gray-300 hover:text-white rounded-lg text-sm font-medium transition-colors"
        >
          <GitBranch className="w-4 h-4" />
          Connect GitLab account
        </button>
      )}
    </div>
  );
}

// ── My Packages Section ────────────────────────────────────────────────────────

function MyPackagesSection() {
  const { user } = useAuthStore();
  const permissions = usePermissions();
  const { data: packages, isLoading } = useQuery({
    queryKey: ["charts", "my"],
    queryFn: () =>
      chartsApi.listByOwner(user?.username ?? "").then((r) => r.data),
    enabled: !!user,
  });

  return (
    <div className="space-y-4">
      <h2 className="text-base font-semibold text-white flex items-center gap-2">
       <Package className="w-4 h-4 text-violet-400" />
       My Packages
      </h2>

      {isLoading ? (
       <div className="space-y-2">
         {[1, 2].map((i) => (
           <div
             key={i}
             className="h-16 bg-gray-800 rounded-lg animate-pulse"
           />
         ))}
       </div>
      ) : packages && packages.length > 0 ? (
       <div className="space-y-3">
         {packages.map((pkg) => (
           <div
             key={pkg.id}
             className="p-4 border border-gray-800 bg-gray-900 rounded-lg hover:border-gray-700 transition-colors"
           >
             <div className="flex justify-between items-start mb-2">
               <div className="flex-1 min-w-0">
                 <h3 className="font-semibold text-white truncate">
                   {pkg.name}
                 </h3>
                 {pkg.description && (
                   <p className="text-sm text-gray-400 truncate">
                     {pkg.description}
                   </p>
                 )}
               </div>
               <div className="ml-4 text-right">
                 <div className="text-2xl font-bold text-violet-400">
                   {pkg.download_count}
                 </div>
                 <div className="text-xs text-gray-500">downloads</div>
               </div>
             </div>

             {permissions.canViewPackageAnalytics(pkg.owner_id) && (
               <button className="text-xs text-violet-400 hover:text-violet-300 mt-2">
                 📊 View Analytics
               </button>
             )}
           </div>
         ))}
       </div>
      ) : (
       <div className="text-center py-10 text-gray-600 border border-gray-800 rounded-lg">
         📦
         <p className="text-sm mt-2">No packages yet.</p>
       </div>
      )}
    </div>
  );
}

// ── Profile ────────────────────────────────────────────────────────────────────

export function Profile() {
  const { user } = useAuthStore();
  const { app_name } = useSettings();
  const qc = useQueryClient();
  const [showModal, setShowModal] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();
  const [githubBanner, setGithubBanner] = useState<
    "connected" | "error" | null
  >(null);
  const [gitlabBanner, setGitlabBanner] = useState<
    "connected" | "error" | null
  >(null);

  // Handle GitHub OAuth redirect result
  useEffect(() => {
    const github = searchParams.get("github");
    if (github === "connected") {
      setGithubBanner("connected");
      qc.invalidateQueries({ queryKey: ["github-connection"] });
    } else if (github === "error") {
      setGithubBanner("error");
    }
    if (github) {
      setSearchParams((p) => {
        p.delete("github");
        p.delete("github_msg");
        return p;
      });
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Handle GitLab OAuth redirect result
  useEffect(() => {
    const gitlab = searchParams.get("gitlab");
    if (gitlab === "connected") {
      setGitlabBanner("connected");
      qc.invalidateQueries({ queryKey: ["gitlab-connection"] });
    } else if (gitlab === "error") {
      setGitlabBanner("error");
    }
    if (gitlab) {
      setSearchParams((p) => {
        p.delete("gitlab");
        p.delete("gitlab_msg");
        return p;
      });
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const { data: tokens = [], isLoading } = useQuery({
    queryKey: ["tokens"],
    queryFn: () => tokensApi.list().then((r) => r.data),
  });

  const revoke = useMutation({
    mutationFn: (id: string) => tokensApi.revoke(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["tokens"] }),
  });

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-8">
      <Seo
        title={`Profile | ${app_name}`}
        description="Manage access tokens, GitHub sync, and account settings."
        canonical={`${window.location.origin}/profile`}
        robots="noindex,follow"
      />
      {showModal && <NewTokenModal onClose={() => setShowModal(false)} />}

      {/* GitHub OAuth banner */}
      {githubBanner === "connected" && (
        <div className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-4 py-3 text-sm">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          GitHub account connected successfully.
          <button
            onClick={() => setGithubBanner(null)}
            className="ml-auto text-emerald-600 hover:text-emerald-400"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}
      {githubBanner === "error" && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-4 py-3 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          GitHub connection failed. Please try again.
          <button
            onClick={() => setGithubBanner(null)}
            className="ml-auto text-red-600 hover:text-red-400"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* GitLab OAuth banner */}
      {gitlabBanner === "connected" && (
        <div className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-4 py-3 text-sm">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          GitLab account connected successfully.
          <button
            onClick={() => setGitlabBanner(null)}
            className="ml-auto text-emerald-600 hover:text-emerald-400"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}
      {gitlabBanner === "error" && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-4 py-3 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          GitLab connection failed. Please try again.
          <button
            onClick={() => setGitlabBanner(null)}
            className="ml-auto text-red-600 hover:text-red-400"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Account info */}
      <div>
        <h1 className="text-2xl font-bold text-white">Profile</h1>
        <div className="mt-4 px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg space-y-1 text-sm">
          <div className="flex justify-between">
            <span className="text-gray-500">Username</span>
            <span className="text-white font-mono">{user?.username}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-500">Email</span>
            <span className="text-gray-300">{user?.email}</span>
          </div>
        </div>
      </div>

      {/* GitHub */}
      <GithubSection />

      {/* GitLab */}
      <GitLabSection />

      {/* My Packages */}
      <MyPackagesSection />

      {/* Access tokens */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-white flex items-center gap-2">
              <Shield className="w-4 h-4 text-violet-400" />
              Personal access tokens
            </h2>
            <p className="text-xs text-gray-500 mt-0.5">
              Use these instead of your password to upload charts from scripts
              or CI.
            </p>
          </div>
          <button
            onClick={() => setShowModal(true)}
            className="flex items-center gap-1.5 bg-violet-600 hover:bg-violet-500 text-white px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            New token
          </button>
        </div>

        {isLoading ? (
          <div className="space-y-2">
            {[1, 2].map((i) => (
              <div
                key={i}
                className="h-16 bg-gray-800 rounded-lg animate-pulse"
              />
            ))}
          </div>
        ) : tokens.length === 0 ? (
          <div className="text-center py-10 text-gray-600 border border-gray-800 rounded-lg">
            <Key className="w-7 h-7 mx-auto mb-2 opacity-40" />
            <p className="text-sm">No tokens yet.</p>
          </div>
        ) : (
          <div className="space-y-2">
            {tokens.map((t) => (
              <TokenRow
                key={t.id}
                token={t}
                onRevoke={() => revoke.mutate(t.id)}
              />
            ))}
          </div>
        )}

        {tokens.length > 0 && (
          <p className="text-xs text-gray-600">
            Pass a token as{" "}
            <code className="text-violet-400">
              Authorization: Bearer hhub_…
            </code>{" "}
            on any authenticated API call.
          </p>
        )}
      </div>
    </div>
  );
}
