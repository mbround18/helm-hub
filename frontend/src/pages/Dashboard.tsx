import { useState, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  Upload,
  Trash2,
  Download,
  ChevronRight,
  Package,
  AlertCircle,
  CheckCircle2,
  Terminal,
  ChevronDown,
  ChevronUp,
  Copy,
  Check,
  X,
  Eye,
} from 'lucide-react'
import { chartsApi, type Chart, type ChartVersion, type UploadResult } from '../lib/api'
import { ChartDetail } from '../components/ChartDetail'
import { useAuthStore } from '../stores/auth'
import clsx from 'clsx'

// ── Confirmation modal ────────────────────────────────────────────────────────

function ConfirmModal({
  title,
  message,
  confirmLabel = 'Delete',
  onConfirm,
  onClose,
}: {
  title: string
  message: string
  confirmLabel?: string
  onConfirm: () => void
  onClose: () => void
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="w-full max-w-sm rounded-xl border border-gray-800 bg-gray-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-800">
          <h2 className="text-base font-semibold text-white">{title}</h2>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="px-5 py-4 space-y-4">
          <p className="text-sm text-gray-300">{message}</p>
          <div className="flex gap-3 justify-end">
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm text-gray-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={() => { onConfirm(); onClose() }}
              className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-sm font-medium rounded-lg transition-colors"
            >
              {confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export function Dashboard() {
  const { user } = useAuthStore()
  const qc = useQueryClient()
  const [selectedChart, setSelectedChart] = useState<Chart | null>(null)
  const [viewChart, setViewChart] = useState<Chart | null>(null)
  const [uploadResult, setUploadResult] = useState<UploadResult | null>(null)
  const [cliOpen, setCliOpen] = useState(false)
  const fileRef = useRef<HTMLInputElement>(null)
  const [confirm, setConfirm] = useState<{
    title: string
    message: string
    confirmLabel?: string
    onConfirm: () => void
  } | null>(null)

  const { data: charts = [], isLoading } = useQuery({
    queryKey: ['charts', user?.username],
    queryFn: () => chartsApi.listByOwner(user!.username).then((r) => r.data),
    enabled: !!user,
  })

  const { data: versions = [] } = useQuery({
    queryKey: ['versions', selectedChart?.id],
    queryFn: () =>
      chartsApi.listVersions(user!.username, selectedChart!.name).then((r) => r.data),
    enabled: !!selectedChart,
  })

  const upload = useMutation({
    mutationFn: (files: File[]) => chartsApi.upload(user!.username, files),
    onSuccess: (res) => {
      setUploadResult(res.data)
      qc.invalidateQueries({ queryKey: ['charts', user?.username] })
    },
    onError: () => setUploadResult(null),
  })

  const deleteVersion = useMutation({
    mutationFn: ({ chartName, version }: { chartName: string; version: string }) =>
      chartsApi.deleteVersion(user!.username, chartName, version),
    onSuccess: (_, { chartName }) => {
      qc.invalidateQueries({ queryKey: ['versions', selectedChart?.id] })
      qc.invalidateQueries({ queryKey: ['charts', user?.username] })
      // If we deleted the last version the chart is gone — deselect it.
      if (selectedChart?.name === chartName) {
        qc.fetchQuery({
          queryKey: ['versions', selectedChart?.id],
          queryFn: () =>
            chartsApi.listVersions(user!.username, chartName).then((r) => r.data),
        }).then((remaining) => {
          if (remaining.length === 0) setSelectedChart(null)
        })
      }
    },
  })

  const purgeChart = useMutation({
    mutationFn: (chartName: string) => chartsApi.purgeChart(user!.username, chartName),
    onSuccess: () => {
      setSelectedChart(null)
      qc.invalidateQueries({ queryKey: ['charts', user?.username] })
    },
  })

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files ?? [])
    if (files.length > 0) {
      setUploadResult(null)
      upload.mutate(files)
    }
    e.target.value = ''
  }

  const hubBase = window.location.origin

  return (
    <div className="p-6 max-w-6xl mx-auto space-y-6">
      {confirm && (
        <ConfirmModal
          {...confirm}
          onClose={() => setConfirm(null)}
        />
      )}

      {/* Chart detail modal */}
      {viewChart && (
        <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/70 p-4 overflow-y-auto">
          <div className="w-full max-w-3xl rounded-xl border border-gray-800 bg-gray-900 shadow-2xl my-8">
            <div className="flex items-center justify-between px-6 py-4 border-b border-gray-800 sticky top-0 bg-gray-900 rounded-t-xl">
              <div className="flex items-center gap-2">
                <Package className="w-4 h-4 text-violet-400" />
                <span className="text-sm font-semibold text-white">{viewChart.name}</span>
              </div>
              <button
                onClick={() => setViewChart(null)}
                className="text-gray-500 hover:text-white transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <ChartDetail
              name={viewChart.name}
              ownerUsername={user!.username}
              description={viewChart.description}
              keywords={viewChart.keywords}
              downloadCount={viewChart.download_count}
            />
          </div>
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">My Charts</h1>
          <p className="text-sm text-gray-400 mt-1">
            Namespace: <code className="text-violet-400">{user?.username}</code>
          </p>
        </div>
        <button
          onClick={() => fileRef.current?.click()}
          disabled={upload.isPending}
          className="flex items-center gap-2 bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors"
        >
          <Upload className="w-4 h-4" />
          {upload.isPending ? 'Uploading…' : 'Upload Charts'}
        </button>
        <input
          ref={fileRef}
          type="file"
          accept=".tgz,.gz"
          multiple
          className="hidden"
          onChange={handleFileChange}
        />
      </div>

      {/* Upload result banner */}
      {upload.isError && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-4 py-3 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {(upload.error as any)?.response?.data?.error ?? 'Upload request failed'}
        </div>
      )}

      {uploadResult && (
        <div className="space-y-2">
          {uploadResult.uploaded.map((u, i) => (
            <div
              key={i}
              className="flex items-center gap-2 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-4 py-3 text-sm"
            >
              <CheckCircle2 className="w-4 h-4 shrink-0" />
              <span>
                <span className="font-mono font-medium">{u.chart}</span>{' '}
                <span className="text-emerald-600">v{u.version}</span> uploaded successfully
              </span>
            </div>
          ))}
          {uploadResult.failed.map((f, i) => (
            <div
              key={i}
              className="flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-4 py-3 text-sm"
            >
              <AlertCircle className="w-4 h-4 shrink-0" />
              {f.error}
            </div>
          ))}
        </div>
      )}

      {/* CLI Usage panel */}
      <div className="border border-gray-800 rounded-lg overflow-hidden">
        <button
          onClick={() => setCliOpen((v) => !v)}
          className="w-full flex items-center justify-between px-4 py-3 text-sm font-medium text-gray-300 hover:text-white hover:bg-gray-900/50 transition-colors"
        >
          <span className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-violet-400" />
            CLI &amp; API usage
          </span>
          {cliOpen ? (
            <ChevronUp className="w-4 h-4 text-gray-500" />
          ) : (
            <ChevronDown className="w-4 h-4 text-gray-500" />
          )}
        </button>

        {cliOpen && (
          <div className="border-t border-gray-800 px-4 py-4 space-y-5 bg-gray-950/60">
            <p className="text-xs text-gray-500">
              All upload operations use standard HTTP multipart. You can push charts directly from CI
              or your local machine without any plugin.
            </p>

            <CliSection title="1. Package your chart">
              <CodeBlock code={`helm package ./my-chart`} />
            </CliSection>

            <CliSection title="2. Authenticate">
              <p className="text-xs text-gray-600 mb-1">
                Option A — personal access token (recommended for automation):
              </p>
              <CodeBlock code={`TOKEN=hhub_<your-token>   # generate one on your Profile page`} />
              <p className="text-xs text-gray-600 mt-2 mb-1">
                Option B — short-lived JWT from username + password:
              </p>
              <CodeBlock
                code={`TOKEN=$(curl -s -X POST \\
  -H "Content-Type: application/json" \\
  -d '{"username":"${user?.username}","password":"<password>"}' \\
  ${hubBase}/api/auth/login | jq -r .token)`}
              />
            </CliSection>

            <CliSection title="3a. Upload a single chart">
              <CodeBlock
                code={`curl -X POST \\
  -H "Authorization: Bearer $TOKEN" \\
  -F "chart=@my-chart-1.0.0.tgz" \\
  ${hubBase}/api/charts/${user?.username}`}
              />
            </CliSection>

            <CliSection title="3b. Upload multiple charts in one request">
              <CodeBlock
                code={`curl -X POST \\
  -H "Authorization: Bearer $TOKEN" \\
  -F "chart=@chart-a-1.0.0.tgz" \\
  -F "chart=@chart-b-2.1.0.tgz" \\
  ${hubBase}/api/charts/${user?.username}`}
              />
            </CliSection>

            <CliSection title="3c. Bulk push all .tgz in a directory">
              <CodeBlock
                code={`for f in *.tgz; do
  curl -s -X POST \\
    -H "Authorization: Bearer $TOKEN" \\
    -F "chart=@$f" \\
    ${hubBase}/api/charts/${user?.username} | jq .
done`}
              />
            </CliSection>

            <CliSection title="4. Download a specific version">
              <CodeBlock
                code={`curl -OJ \\
  ${hubBase}/api/charts/${user?.username}/<chart>/<version>/download`}
              />
            </CliSection>
          </div>
        )}
      </div>

      {/* Chart list + versions */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-1 space-y-2">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-3">
            Charts ({charts.length})
          </h2>

          {isLoading ? (
            <SkeletonList />
          ) : charts.length === 0 ? (
            <EmptyState />
          ) : (
            charts.map((chart) => (
              <div
                key={chart.id}
                className={clsx(
                  'rounded-lg border transition-colors',
                  selectedChart?.id === chart.id
                    ? 'bg-violet-950/50 border-violet-700'
                    : 'bg-gray-900 border-gray-800 hover:border-gray-700',
                )}
              >
                <button
                  onClick={() => setSelectedChart(chart)}
                  className="w-full text-left px-4 py-3"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 min-w-0">
                      <Package className="w-4 h-4 text-violet-400 shrink-0" />
                      <span className={clsx(
                        'font-medium text-sm truncate',
                        selectedChart?.id === chart.id ? 'text-white' : 'text-gray-300',
                      )}>
                        {chart.name}
                      </span>
                    </div>
                    <div className="flex items-center gap-1 shrink-0">
                      <button
                        onClick={(e) => { e.stopPropagation(); setViewChart(chart) }}
                        className="p-1 text-gray-600 hover:text-violet-400 hover:bg-violet-950/40 rounded transition-colors"
                        title="View install instructions"
                      >
                        <Eye className="w-3.5 h-3.5" />
                      </button>
                      <ChevronRight className="w-3 h-3 text-gray-600" />
                    </div>
                  </div>
                  {chart.description && (
                    <p className="text-xs text-gray-500 mt-1 truncate">{chart.description}</p>
                  )}
                </button>
              </div>
            ))
          )}
        </div>

        <div className="lg:col-span-2">
          {selectedChart ? (
            <>
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500">
                  {selectedChart.name} — versions
                </h2>
                <button
                  onClick={() =>
                    setConfirm({
                      title: `Purge ${selectedChart.name}`,
                      message: `This will permanently delete all versions of "${selectedChart.name}" and remove it from your chart list. This cannot be undone.`,
                      confirmLabel: 'Purge',
                      onConfirm: () => purgeChart.mutate(selectedChart.name),
                    })
                  }
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-red-400 hover:text-white hover:bg-red-600 border border-red-800 hover:border-red-600 rounded-lg transition-colors"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  Purge chart
                </button>
              </div>
              <div className="space-y-2">
                {versions.map((v) => (
                  <VersionRow
                    key={v.id}
                    version={v}
                    chartName={selectedChart.name}
                    owner={user!.username}
                    onDelete={() =>
                      setConfirm({
                        title: `Delete v${v.version}`,
                        message: `Delete version ${v.version} of "${selectedChart.name}"? The .tgz file will be removed from storage.`,
                        onConfirm: () =>
                          deleteVersion.mutate({ chartName: selectedChart.name, version: v.version }),
                      })
                    }
                  />
                ))}
              </div>
            </>
          ) : (
            <div className="h-full flex items-center justify-center text-gray-600 text-sm">
              Select a chart to view versions
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function CliSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-gray-400">{title}</p>
      {children}
    </div>
  )
}

function CodeBlock({ code }: { code: string }) {
  const [copied, setCopied] = useState(false)

  const copy = () => {
    navigator.clipboard.writeText(code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="relative group">
      <pre className="bg-gray-900 border border-gray-800 rounded-md px-4 py-3 text-xs text-gray-300 font-mono overflow-x-auto whitespace-pre">
        {code}
      </pre>
      <button
        onClick={copy}
        className="absolute top-2 right-2 p-1.5 rounded text-gray-600 hover:text-gray-300 hover:bg-gray-800 opacity-0 group-hover:opacity-100 transition-all"
        title="Copy"
      >
        {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
      </button>
    </div>
  )
}

function VersionRow({
  version,
  chartName,
  owner,
  onDelete,
}: {
  version: ChartVersion
  chartName: string
  owner: string
  onDelete: () => void
}) {
  return (
    <div className="flex items-center justify-between px-4 py-3 bg-gray-900 border border-gray-800 rounded-lg">
      <div>
        <div className="flex items-center gap-2">
          <span className="text-sm font-mono text-white">{version.version}</span>
          {version.app_version && (
            <span className="text-xs text-gray-500">app: {version.app_version}</span>
          )}
          {version.deprecated !== 0 && (
            <span className="text-xs bg-yellow-900/50 text-yellow-400 border border-yellow-800 px-1.5 py-0.5 rounded">
              deprecated
            </span>
          )}
        </div>
        <p className="text-xs text-gray-500 mt-0.5 font-mono">{version.digest.slice(0, 19)}…</p>
      </div>
      <div className="flex items-center gap-2">
        <a
          href={chartsApi.downloadUrl(owner, chartName, version.version)}
          className="p-1.5 text-gray-400 hover:text-white hover:bg-gray-800 rounded transition-colors"
          title="Download"
        >
          <Download className="w-4 h-4" />
        </a>
        <button
          onClick={onDelete}
          className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-gray-800 rounded transition-colors"
          title="Delete"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}

function EmptyState() {
  return (
    <div className="text-center py-12 text-gray-600">
      <Package className="w-8 h-8 mx-auto mb-2 opacity-40" />
      <p className="text-sm">No charts yet. Upload your first .tgz!</p>
    </div>
  )
}

function SkeletonList() {
  return (
    <>
      {[1, 2, 3].map((i) => (
        <div key={i} className="h-14 bg-gray-800 rounded-lg animate-pulse" />
      ))}
    </>
  )
}
