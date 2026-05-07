import axios from 'axios'

export const api = axios.create({
  baseURL: '/api',
  headers: { 'Content-Type': 'application/json' },
})

// Attach JWT from localStorage on every request
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('token')
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

// ── Types ─────────────────────────────────────────────────────────────────────

export interface User {
  id: string
  username: string
  email: string
  totp_enabled: number
  is_admin: number
  created_at: string
  updated_at: string
}

export interface Chart {
  id: string
  owner_id: string
  name: string
  description: string | null
  home_url: string | null
  icon_url: string | null
  keywords: string | null
  is_private: number
  created_at: string
  updated_at: string
}

export interface ChartVersion {
  id: string
  chart_id: string
  version: string
  app_version: string | null
  description: string | null
  digest: string
  storage_path: string
  chart_yaml: string
  values_yaml: string | null
  schema_json: string | null
  deprecated: number
  created_at: string
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export const authApi = {
  register: (username: string, email: string, password: string) =>
    api.post<{ user: User }>('/auth/register', { username, email, password }),

  login: (username: string, password: string, totp_code?: string) =>
    api.post<{ token: string; user: User }>('/auth/login', { username, password, totp_code }),

  totpSetup: () => api.post<{ secret: string; provisioning_uri: string }>('/auth/totp/setup'),

  totpEnable: (code: string) => api.post('/auth/totp/enable', { code }),
}

// ── Charts ────────────────────────────────────────────────────────────────────

export const chartsApi = {
  list: (params?: { q?: string; page?: number; per_page?: number }) =>
    api.get<Chart[]>('/charts', { params }),

  listByOwner: (owner: string) =>
    api.get<Chart[]>(`/charts/${owner}`),

  listVersions: (owner: string, chartName: string) =>
    api.get<ChartVersion[]>(`/charts/${owner}/${chartName}`),

  upload: (owner: string, file: File) => {
    const form = new FormData()
    form.append('chart', file)
    return api.post<{ message: string; chart: string; version: string }>(`/charts/${owner}`, form, {
      headers: { 'Content-Type': 'multipart/form-data' },
    })
  },

  deleteVersion: (owner: string, chartName: string, version: string) =>
    api.delete(`/charts/${owner}/${chartName}/${version}`),

  downloadUrl: (owner: string, chartName: string, version: string) =>
    `/api/charts/${owner}/${chartName}/${version}/download`,
}
