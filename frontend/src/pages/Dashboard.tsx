import { useState, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Upload, Trash2, Download, ChevronRight, Package, AlertCircle } from 'lucide-react'
import { chartsApi, type Chart, type ChartVersion } from '../lib/api'
import { useAuthStore } from '../stores/auth'
import clsx from 'clsx'

export function Dashboard() {
  const { user } = useAuthStore()
  const qc = useQueryClient()
  const [selectedChart, setSelectedChart] = useState<Chart | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

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
    mutationFn: (file: File) => chartsApi.upload(user!.username, file),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['charts', user?.username] }),
  })

  const deleteVersion = useMutation({
    mutationFn: ({ chartName, version }: { chartName: string; version: string }) =>
      chartsApi.deleteVersion(user!.username, chartName, version),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['versions', selectedChart?.id] })
      qc.invalidateQueries({ queryKey: ['charts', user?.username] })
    },
  })

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) upload.mutate(file)
    e.target.value = ''
  }

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-8">
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
          {upload.isPending ? 'Uploading…' : 'Upload Chart'}
        </button>
        <input
          ref={fileRef}
          type="file"
          accept=".tgz,.gz"
          className="hidden"
          onChange={handleFileChange}
        />
      </div>

      {upload.isError && (
        <div className="mb-4 flex items-center gap-2 text-red-400 bg-red-950/40 border border-red-800 rounded-lg px-4 py-3 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {(upload.error as any)?.response?.data?.error ?? 'Upload failed'}
        </div>
      )}

      {upload.isSuccess && (
        <div className="mb-4 text-emerald-400 bg-emerald-950/40 border border-emerald-800 rounded-lg px-4 py-3 text-sm">
          Chart uploaded successfully.
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Chart list */}
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
              <button
                key={chart.id}
                onClick={() => setSelectedChart(chart)}
                className={clsx(
                  'w-full text-left px-4 py-3 rounded-lg border transition-colors',
                  selectedChart?.id === chart.id
                    ? 'bg-violet-950/50 border-violet-700 text-white'
                    : 'bg-gray-900 border-gray-800 text-gray-300 hover:border-gray-700 hover:text-white'
                )}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Package className="w-4 h-4 text-violet-400 shrink-0" />
                    <span className="font-medium text-sm truncate">{chart.name}</span>
                  </div>
                  <ChevronRight className="w-3 h-3 text-gray-600" />
                </div>
                {chart.description && (
                  <p className="text-xs text-gray-500 mt-1 truncate">{chart.description}</p>
                )}
              </button>
            ))
          )}
        </div>

        {/* Version list */}
        <div className="lg:col-span-2">
          {selectedChart ? (
            <>
              <h2 className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-3">
                {selectedChart.name} — versions
              </h2>
              <div className="space-y-2">
                {versions.map((v) => (
                  <VersionRow
                    key={v.id}
                    version={v}
                    chartName={selectedChart.name}
                    owner={user!.username}
                    onDelete={() =>
                      deleteVersion.mutate({ chartName: selectedChart.name, version: v.version })
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
