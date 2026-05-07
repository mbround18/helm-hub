import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Search, Package, User } from 'lucide-react'
import { chartsApi, type Chart } from '../lib/api'
import { Link } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'

export function Explore() {
  const [query, setQuery] = useState('')
  const { data: charts = [], isLoading } = useQuery({
    queryKey: ['charts', 'explore', query],
    queryFn: () => chartsApi.list({ q: query || undefined }).then((r) => r.data),
  })

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white mb-1">Explore Charts</h1>
        <p className="text-gray-400 text-sm">Browse publicly available Helm charts</p>
      </div>

      <div className="relative mb-6">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" />
        <input
          type="text"
          placeholder="Search charts…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full bg-gray-900 border border-gray-800 text-white placeholder-gray-500 rounded-lg pl-10 pr-4 py-2.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
        />
      </div>

      {isLoading ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-24 bg-gray-800 rounded-lg animate-pulse" />
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          {charts.map((chart) => (
            <ChartCard key={chart.id} chart={chart} />
          ))}
        </div>
      )}

      {!isLoading && charts.length === 0 && (
        <p className="text-center text-gray-600 py-16 text-sm">No charts found.</p>
      )}
    </div>
  )
}

function ChartCard({ chart }: { chart: Chart }) {
  const { data: users } = useQuery({
    queryKey: ['user', chart.owner_id],
    queryFn: () => null, // owner username is not directly available — would need an enriched endpoint
    enabled: false,
  })

  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-4 hover:border-gray-700 transition-colors">
      <div className="flex items-start gap-3">
        <Package className="w-5 h-5 text-violet-400 shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          <h3 className="text-sm font-semibold text-white truncate">{chart.name}</h3>
          {chart.description && (
            <p className="text-xs text-gray-400 mt-0.5 line-clamp-2">{chart.description}</p>
          )}
          <div className="flex items-center gap-1 mt-2 text-xs text-gray-600">
            <User className="w-3 h-3" />
            <span className="truncate">{chart.owner_id.slice(0, 8)}</span>
          </div>
        </div>
      </div>
    </div>
  )
}
