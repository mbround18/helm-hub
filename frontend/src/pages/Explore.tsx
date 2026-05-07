import { useState, useMemo, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Document as FlexDocument } from "flexsearch";
import {
  Search,
  Package,
  Download,
  X,
  ArrowLeft,
  TrendingUp,
  Loader2,
} from "lucide-react";
import { chartsApi, type PublicChart } from "../lib/api";
import { ChartDetail, fmtDownloads } from "../components/ChartDetail";
import { AppLogo } from "../components/AppLogo";
import { useSettings } from "../hooks/useSettings";
import clsx from "clsx";

// ── FlexSearch setup ──────────────────────────────────────────────────────────

// FlexSearch's DocumentData constraint (`{ [key: string]: DocumentValue }`) is
// incompatible with plain interfaces, so we use `any` for the index generic and
// cast results back to PublicChart at the use site.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function buildIndex(charts: PublicChart[]): FlexDocument<any> {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const idx = new FlexDocument<any>({
    document: {
      id: "id",
      store: true, // required for enrich:true to populate doc in search results
      index: [
        { field: "name", tokenize: "forward", resolution: 9 },
        { field: "owner_username", tokenize: "forward", resolution: 6 },
        { field: "description", tokenize: "forward", resolution: 3 },
      ],
    },
    encoder: "Default",
    cache: 100,
  });
  for (const chart of charts) idx.add(chart);
  return idx;
}

// ── Search suggestion item ────────────────────────────────────────────────────

function SuggestionItem({
  chart,
  onSelect,
}: {
  chart: PublicChart;
  onSelect: () => void;
}) {
  return (
    <button
      onMouseDown={(e) => {
        e.preventDefault();
        onSelect();
      }}
      className="w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800 transition-colors text-left group"
    >
      <div className="w-8 h-8 rounded-lg bg-violet-950/60 border border-violet-800/30 flex items-center justify-center shrink-0">
        <Package className="w-4 h-4 text-violet-400" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-white truncate">
            {chart.name}
          </span>
          <span className="text-xs text-gray-600 shrink-0">
            by {chart.owner_username}
          </span>
        </div>
        {chart.description && (
          <p className="text-xs text-gray-500 truncate mt-0.5">
            {chart.description}
          </p>
        )}
      </div>
      {chart.download_count > 0 && (
        <span className="flex items-center gap-1 text-xs text-gray-600 shrink-0">
          <Download className="w-3 h-3" />
          {fmtDownloads(chart.download_count)}
        </span>
      )}
    </button>
  );
}

// ── Chart card (grid) ─────────────────────────────────────────────────────────

function ChartCard({
  chart,
  onSelect,
}: {
  chart: PublicChart;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      className="w-full text-left bg-gray-900 hover:bg-gray-800 border border-gray-800 hover:border-gray-700 rounded-xl p-5 transition-all group"
    >
      <div className="flex items-start gap-3 mb-3">
        <div className="w-10 h-10 rounded-lg bg-violet-950/60 border border-violet-800/30 flex items-center justify-center shrink-0 group-hover:border-violet-700/50 transition-colors">
          <Package className="w-5 h-5 text-violet-400" />
        </div>
        <div className="min-w-0 flex-1 pt-0.5">
          <h3 className="text-sm font-semibold text-white truncate">
            {chart.name}
          </h3>
          <span className="text-xs text-gray-600">{chart.owner_username}</span>
        </div>
      </div>
      {chart.description && (
        <p className="text-xs text-gray-400 line-clamp-2 mb-3">
          {chart.description}
        </p>
      )}
      <div className="flex items-center justify-between mt-auto">
        {chart.keywords ? (
          <div className="flex items-center gap-1 flex-wrap min-w-0">
            {chart.keywords
              .split(",")
              .slice(0, 2)
              .map((kw) => (
                <span
                  key={kw.trim()}
                  className="text-xs bg-gray-800 text-gray-500 px-1.5 py-0.5 rounded truncate"
                >
                  {kw.trim()}
                </span>
              ))}
          </div>
        ) : (
          <span />
        )}
        {chart.download_count > 0 && (
          <span className="flex items-center gap-1 text-xs text-gray-600 shrink-0">
            <Download className="w-3 h-3" />
            {fmtDownloads(chart.download_count)}
          </span>
        )}
      </div>
    </button>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export function Explore() {
  const [query, setQuery] = useState("");
  const [focused, setFocused] = useState(false);
  const [selected, setSelected] = useState<PublicChart | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const { app_name } = useSettings();

  // Load all charts (sorted by downloads) for Fuse.js index
  const { data: allCharts = [], isLoading } = useQuery({
    queryKey: ["charts", "explore-all"],
    queryFn: () => chartsApi.list({ per_page: 200 }).then((r) => r.data),
    staleTime: 60_000,
  });

  // FlexSearch index — rebuilt only when chart data changes
  const index = useMemo(() => buildIndex(allCharts), [allCharts]);

  // Search results (min 3 chars) — deduplicated across per-field result sets
  const results = useMemo<PublicChart[]>(() => {
    if (query.length < 3) return [];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const raw: any[] = index.search(query, { limit: 8, enrich: true });
    const seen = new Set<string>();
    const out: PublicChart[] = [];
    for (const { result } of raw) {
      for (const { id, doc } of result) {
        if (doc && !seen.has(String(id))) {
          seen.add(String(id));
          out.push(doc as PublicChart);
          if (out.length >= 8) return out;
        }
      }
    }
    return out;
  }, [index, query]);

  // Cards shown below the search bar
  const gridCharts = useMemo<PublicChart[]>(() => {
    if (query.length >= 3) return results;
    return allCharts.slice(0, 24); // top 24 by downloads when idle
  }, [query, results, allCharts]);

  const showDropdown = focused && query.length >= 3 && results.length > 0;
  const hasQuery = query.length >= 3;

  const selectChart = (chart: PublicChart) => {
    setSelected(chart);
    setFocused(false);
  };

  const clearSearch = () => {
    setQuery("");
    setSelected(null);
    inputRef.current?.focus();
  };

  // ── Selected chart view ──────────────────────────────────────────────────────
  if (selected) {
    return (
      <div className="min-h-full">
        {/* Compact top bar */}
        <div className="sticky top-0 z-10 bg-gray-950/90 backdrop-blur border-b border-gray-800 px-6 py-3 flex items-center gap-4">
          <button
            onClick={() => setSelected(null)}
            className="flex items-center gap-1.5 text-sm text-gray-400 hover:text-white transition-colors shrink-0"
          >
            <ArrowLeft className="w-4 h-4" />
            Explore
          </button>
          <div className="relative flex-1 max-w-lg">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" />
            <input
              ref={inputRef}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSelected(null);
              }}
              onFocus={() => setFocused(true)}
              onBlur={() => setTimeout(() => setFocused(false), 100)}
              placeholder="Search charts…"
              className="w-full bg-gray-900 border border-gray-800 text-white placeholder-gray-600 rounded-lg pl-9 pr-4 py-1.5 text-sm focus:outline-none focus:border-violet-600 transition-colors"
            />
            {showDropdown && (
              <SearchDropdown results={results} onSelect={selectChart} />
            )}
          </div>
        </div>
        <ChartDetail
          name={selected.name}
          ownerUsername={selected.owner_username}
          description={selected.description}
          keywords={selected.keywords}
          downloadCount={selected.download_count}
        />
      </div>
    );
  }

  // ── Hero / search view ───────────────────────────────────────────────────────
  return (
    <div className="min-h-full">
      {/* Hero */}
      <div
        className={clsx(
          "transition-all duration-300 text-center px-6",
          hasQuery ? "pt-10 pb-6" : "pt-24 pb-10",
        )}
      >
        {!hasQuery && (
          <>
            <div className="flex justify-center mb-5">
              <AppLogo
                size="lg"
                showName={false}
                className="text-violet-400 w-16 h-16 rounded-2xl bg-violet-950/60 border border-violet-800/50 justify-center"
              />
            </div>
            <h1 className="text-3xl font-bold text-white mb-2">{app_name}</h1>
            <p className="text-gray-400 text-sm mb-8">
              Discover, install, and share Helm charts
            </p>
          </>
        )}

        {/* Search bar */}
        <div
          className={clsx(
            "relative mx-auto transition-all duration-300",
            hasQuery ? "max-w-2xl" : "max-w-xl",
          )}
        >
          <div
            className={clsx(
              "flex items-center gap-3 bg-gray-900 border rounded-2xl px-4 py-3 transition-all duration-200",
              focused
                ? "border-violet-600 shadow-lg shadow-violet-900/20"
                : "border-gray-800",
            )}
          >
            {isLoading ? (
              <Loader2 className="w-5 h-5 text-gray-500 shrink-0 animate-spin" />
            ) : (
              <Search className="w-5 h-5 text-gray-500 shrink-0" />
            )}
            <input
              ref={inputRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onFocus={() => setFocused(true)}
              onBlur={() => setTimeout(() => setFocused(false), 100)}
              placeholder="Search charts, users… (min. 3 characters)"
              className="flex-1 bg-transparent text-white placeholder-gray-600 text-sm focus:outline-none"
              autoComplete="off"
              spellCheck={false}
            />
            {query && (
              <button
                onClick={clearSearch}
                className="text-gray-600 hover:text-gray-400 transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>

          {/* Suggestion dropdown */}
          {showDropdown && (
            <SearchDropdown results={results} onSelect={selectChart} />
          )}

          {/* Min-chars hint */}
          {focused && query.length > 0 && query.length < 3 && (
            <p className="text-xs text-gray-600 mt-2">
              Type {3 - query.length} more character
              {3 - query.length !== 1 ? "s" : ""}…
            </p>
          )}
        </div>
      </div>

      {/* Grid label */}
      <div className="px-6 max-w-6xl mx-auto">
        <div className="flex items-center gap-2 mb-4">
          {hasQuery ? (
            <>
              <Search className="w-3.5 h-3.5 text-gray-600" />
              <span className="text-xs text-gray-500">
                {results.length} result{results.length !== 1 ? "s" : ""} for{" "}
                <span className="text-gray-300">"{query}"</span>
              </span>
            </>
          ) : (
            <>
              <TrendingUp className="w-3.5 h-3.5 text-gray-600" />
              <span className="text-xs text-gray-500">Popular charts</span>
            </>
          )}
        </div>

        {/* Chart grid */}
        {isLoading ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {Array.from({ length: 8 }).map((_, i) => (
              <div
                key={i}
                className="h-36 bg-gray-800 rounded-xl animate-pulse"
              />
            ))}
          </div>
        ) : gridCharts.length === 0 ? (
          <div className="text-center py-20 text-gray-600">
            <Package className="w-10 h-10 mx-auto mb-3 opacity-30" />
            <p className="text-sm">No charts found for "{query}"</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {gridCharts.map((chart) => (
              <ChartCard
                key={chart.id}
                chart={chart}
                onSelect={() => selectChart(chart)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Dropdown (shared between hero and compact bar) ────────────────────────────

function SearchDropdown({
  results,
  onSelect,
}: {
  results: PublicChart[];
  onSelect: (c: PublicChart) => void;
}) {
  return (
    <div className="absolute top-full mt-2 left-0 right-0 bg-gray-900 border border-gray-800 rounded-xl shadow-2xl overflow-hidden z-50">
      <div className="divide-y divide-gray-800/50">
        {results.map((chart) => (
          <SuggestionItem
            key={chart.id}
            chart={chart}
            onSelect={() => onSelect(chart)}
          />
        ))}
      </div>
    </div>
  );
}
