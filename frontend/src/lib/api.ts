import axios from "axios";

export const api = axios.create({
  baseURL: "/api",
  headers: { "Content-Type": "application/json" },
});

// Attach JWT from localStorage on every request
api.interceptors.request.use((config) => {
  const token = localStorage.getItem("token");
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

// ── Types ─────────────────────────────────────────────────────────────────────

// ── API Token types ───────────────────────────────────────────────────────────

export type TokenTtlDays = 30 | 60 | 90 | 180 | 365;

export interface ApiToken {
  id: string;
  user_id: string;
  description: string;
  expires_at: string;
  last_used_at: string | null;
  created_at: string;
}

export interface CreatedToken extends ApiToken {
  /** Raw token value — returned exactly once at creation time. */
  token: string;
}

export interface UploadedChart {
  chart: string;
  version: string;
  owner: string;
}

export interface FailedChart {
  error: string;
}

export interface UploadResult {
  uploaded: UploadedChart[];
  failed: FailedChart[];
}

export interface User {
  id: string;
  username: string;
  email: string;
  totp_enabled: number;
  is_admin: number;
  created_at: string;
  updated_at: string;
}

export interface Chart {
  id: string;
  owner_id: string;
  name: string;
  description: string | null;
  home_url: string | null;
  icon_url: string | null;
  keywords: string | null;
  is_private: number;
  created_at: string;
  updated_at: string;
  download_count: number;
}

/** Returned by GET /api/charts — includes owner_username for install URLs. */
export interface PublicChart extends Chart {
  owner_username: string;
}

export interface ChartVersion {
  id: string;
  chart_id: string;
  version: string;
  app_version: string | null;
  description: string | null;
  digest: string;
  storage_path: string;
  chart_yaml: string;
  values_yaml: string | null;
  schema_json: string | null;
  deprecated: number;
  created_at: string;
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export const authApi = {
  register: (username: string, email: string, password: string) =>
    api.post<{ user: User }>("/auth/register", { username, email, password }),

  login: (username: string, password: string, totp_code?: string) =>
    api.post<{ token: string; user: User }>("/auth/login", {
      username,
      password,
      totp_code,
    }),

  totpSetup: () =>
    api.post<{ secret: string; provisioning_uri: string }>("/auth/totp/setup"),

  totpEnable: (code: string) => api.post("/auth/totp/enable", { code }),
};

// ── Charts ────────────────────────────────────────────────────────────────────

export const chartsApi = {
  list: (params?: { q?: string; page?: number; per_page?: number }) =>
    api.get<PublicChart[]>("/charts", { params }),

  listByOwner: (owner: string) => api.get<Chart[]>(`/charts/${owner}`),

  listVersions: (owner: string, chartName: string) =>
    api.get<ChartVersion[]>(`/charts/${owner}/${chartName}`),

  upload: (owner: string, files: File | File[]) => {
    const form = new FormData();
    const list = Array.isArray(files) ? files : [files];
    for (const f of list) form.append("chart", f);
    return api.post<UploadResult>(`/charts/${owner}`, form, {
      headers: { "Content-Type": "multipart/form-data" },
    });
  },

  deleteVersion: (owner: string, chartName: string, version: string) =>
    api.delete(`/charts/${owner}/${chartName}/${version}`),

  purgeChart: (owner: string, chartName: string) =>
    api.delete(`/charts/${owner}/${chartName}`),

  downloadUrl: (owner: string, chartName: string, version: string) =>
    `/api/charts/${owner}/${chartName}/${version}/download`,
};

// ── GitHub ────────────────────────────────────────────────────────────────────

export interface GithubConnection {
  id: string;
  user_id: string;
  github_id: string;
  github_username: string;
  avatar_url: string | null;
  created_at: string;
  updated_at: string;
}

export interface GithubRepo {
  id: string;
  user_id: string;
  github_connection_id: string;
  repo_owner: string;
  repo_name: string;
  last_synced_at: string | null;
  created_at: string;
}

export interface ChartSyncEntry {
  chart: string;
  version: string;
  status: "imported" | "skipped" | "failed";
  message?: string;
}

export interface SyncReport {
  repo: string;
  entries: ChartSyncEntry[];
  synced_at: string;
}

export const githubApi = {
  /** Returns the GitHub OAuth authorization URL. Must be logged in. */
  oauthUrl: (returnTo: string) =>
    api.get<{ url: string }>(
      `/auth/github/url?return_to=${encodeURIComponent(returnTo)}`,
    ),

  getConnection: () =>
    api.get<{ connection: GithubConnection | null }>("/github/connection"),

  deleteConnection: () => api.delete("/github/connection"),

  listRepos: () => api.get<GithubRepo[]>("/github/repos"),

  addRepo: (repo: string) => api.post<GithubRepo>("/github/repos", { repo }),

  removeRepo: (id: string) => api.delete(`/github/repos/${id}`),

  syncRepo: (id: string) => api.post<SyncReport>(`/github/repos/${id}/sync`),
};

// ── Personal Access Tokens ────────────────────────────────────────────────────

export const tokensApi = {
  list: () => api.get<ApiToken[]>("/tokens"),

  create: (description: string, ttl_days: TokenTtlDays) =>
    api.post<CreatedToken>("/tokens", { description, ttl_days }),

  revoke: (id: string) => api.delete(`/tokens/${id}`),
};

// ── App Settings ──────────────────────────────────────────────────────────────

export interface AppSettings {
  app_name: string;
  logo_url: string;
  signup_enabled: boolean;
}

export const settingsApi = {
  get: () => api.get<AppSettings>("/settings"),
  update: (
    body: Partial<{
      app_name: string;
      logo_url: string;
      signup_enabled: boolean;
    }>,
  ) => api.put("/admin/settings", body),
};

// ── Admin ─────────────────────────────────────────────────────────────────────

export interface AdminUser {
  id: string;
  username: string;
  email: string;
  is_admin: number;
  banned_at: string | null;
  storage_usage_bytes: number;
  storage_quota_bytes: number | null;
  created_at: string;
}

export const adminApi = {
  listUsers: () => api.get<AdminUser[]>("/admin/users"),
  promoteUser: (id: string) => api.post(`/admin/users/${id}/promote`),
  banUser: (id: string) => api.post(`/admin/users/${id}/ban`),
  purgeUser: (id: string) => api.delete(`/admin/users/${id}`),
  setQuota: (id: string, quota_bytes: number | null) =>
    api.put(`/admin/users/${id}/quota`, { quota_bytes }),
  deleteChart: (owner: string, chartName: string) =>
    api.delete(`/admin/charts/${owner}/${chartName}`),
};
