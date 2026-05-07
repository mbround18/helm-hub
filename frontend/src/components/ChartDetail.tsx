import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Package,
  Download,
  User,
  Tag,
  Copy,
  Check,
  ChevronDown,
  Terminal,
  FileText,
  Loader2,
  TrendingUp,
} from "lucide-react";
import { chartsApi, type ChartVersion } from "../lib/api";
import clsx from "clsx";

// ── Helpers ───────────────────────────────────────────────────────────────────

export function fmtDownloads(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 10_000 ? 0 : 1)}k`;
  return String(n);
}

export function CodeBlock({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="relative group">
      <pre className="bg-gray-950 border border-gray-800 rounded-lg px-4 py-3 text-xs font-mono text-gray-300 overflow-x-auto whitespace-pre">
        {code}
      </pre>
      <button
        onClick={() => {
          navigator.clipboard.writeText(code);
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        }}
        className="absolute top-2 right-2 p-1.5 rounded text-gray-600 hover:text-gray-300 hover:bg-gray-800 opacity-0 group-hover:opacity-100 transition-all"
      >
        {copied ? (
          <Check className="w-3.5 h-3.5 text-emerald-400" />
        ) : (
          <Copy className="w-3.5 h-3.5" />
        )}
      </button>
    </div>
  );
}

// ── ChartDetail ───────────────────────────────────────────────────────────────

type DetailTab = "install" | "values";

export interface ChartDetailProps {
  name: string;
  ownerUsername: string;
  description?: string | null;
  keywords?: string | null;
  downloadCount: number;
}

export function ChartDetail({
  name,
  ownerUsername,
  description,
  keywords,
  downloadCount,
}: ChartDetailProps) {
  const [tab, setTab] = useState<DetailTab>("install");
  const [selectedVersion, setSelectedVersion] = useState<string | null>(null);

  const { data: versions = [] as ChartVersion[], isLoading } = useQuery({
    queryKey: ["versions-detail", ownerUsername, name],
    queryFn: (): Promise<ChartVersion[]> =>
      chartsApi.listVersions(ownerUsername, name).then((r) => r.data),
  });

  useEffect(() => {
    if (versions.length > 0 && !selectedVersion) {
      setSelectedVersion(versions[0].version);
    }
  }, [versions]); // eslint-disable-line react-hooks/exhaustive-deps

  const active =
    versions.find((v) => v.version === selectedVersion) ?? versions[0];
  const base = window.location.origin;
  const repoAlias = `hh-${ownerUsername}`;
  const repoUrl = `${base}/api/charts/${ownerUsername}`;

  return (
    <div className="max-w-3xl mx-auto px-6 py-8 space-y-6">
      {/* Chart header */}
      <div className="flex items-start gap-4">
        <div className="w-14 h-14 rounded-xl bg-violet-950/60 border border-violet-800/40 flex items-center justify-center shrink-0">
          <Package className="w-7 h-7 text-violet-400" />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="text-2xl font-bold text-white">{name}</h2>
          {description && (
            <p className="text-gray-400 text-sm mt-1">{description}</p>
          )}
          <div className="flex items-center gap-4 mt-2 flex-wrap">
            <span className="flex items-center gap-1 text-xs text-gray-500">
              <User className="w-3 h-3" /> {ownerUsername}
            </span>
            <span className="flex items-center gap-1 text-xs text-gray-500">
              <TrendingUp className="w-3 h-3" />
              <Download className="w-3 h-3" /> {fmtDownloads(downloadCount)}{" "}
              downloads
            </span>
            {keywords &&
              keywords
                .split(",")
                .slice(0, 5)
                .map((kw) => (
                  <span
                    key={kw.trim()}
                    className="flex items-center gap-0.5 text-xs bg-gray-800 text-gray-400 px-1.5 py-0.5 rounded"
                  >
                    <Tag className="w-2.5 h-2.5" />
                    {kw.trim()}
                  </span>
                ))}
          </div>
        </div>
      </div>

      {/* Version picker */}
      {!isLoading && versions.length > 0 && (
        <div className="flex items-center gap-3">
          <span className="text-xs text-gray-500">Version</span>
          <div className="relative">
            <select
              value={active?.version ?? ""}
              onChange={(e) => setSelectedVersion(e.target.value)}
              className="appearance-none bg-gray-800 border border-gray-700 text-white text-xs font-mono rounded-lg pl-3 pr-8 py-1.5 focus:outline-none focus:border-violet-500 cursor-pointer"
            >
              {versions.map((v, i) => (
                <option key={v.id} value={v.version}>
                  {v.version}
                  {i === 0 ? " (latest)" : ""}
                </option>
              ))}
            </select>
            <ChevronDown className="absolute right-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400 pointer-events-none" />
          </div>
          {active?.app_version && (
            <span className="text-xs text-gray-600">
              app: {active.app_version}
            </span>
          )}
          {active?.deprecated !== 0 && (
            <span className="text-xs bg-yellow-900/50 text-yellow-400 border border-yellow-800 px-1.5 py-0.5 rounded">
              deprecated
            </span>
          )}
        </div>
      )}

      {/* Tabs */}
      <div className="border-b border-gray-800">
        <div className="flex">
          {(
            [
              ["install", "Install", Terminal],
              ["values", "Values", FileText],
            ] as const
          ).map(([id, label, Icon]) => (
            <button
              key={id}
              onClick={() => setTab(id as DetailTab)}
              className={clsx(
                "flex items-center gap-1.5 px-4 py-2.5 text-xs font-medium border-b-2 transition-colors",
                tab === id
                  ? "border-violet-500 text-violet-400"
                  : "border-transparent text-gray-500 hover:text-gray-300",
              )}
            >
              <Icon className="w-3.5 h-3.5" />
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content */}
      {isLoading ? (
        <div className="flex items-center gap-2 text-gray-600 text-sm py-8 justify-center">
          <Loader2 className="w-4 h-4 animate-spin" /> Loading versions…
        </div>
      ) : !active ? (
        <p className="text-gray-600 text-sm text-center py-8">
          No versions available.
        </p>
      ) : tab === "install" ? (
        <div className="space-y-5">
          {/* Helm repo add — the recommended path for ArgoCD / FluxCD */}
          <div>
            <p className="text-xs font-medium text-gray-400 mb-1.5">
              1. Add the Helm repository
            </p>
            <CodeBlock
              code={`helm repo add ${repoAlias} ${repoUrl}\nhelm repo update`}
            />
          </div>
          <div>
            <p className="text-xs font-medium text-gray-400 mb-1.5">
              2. Install
            </p>
            <CodeBlock
              code={`helm install my-release ${repoAlias}/${name} --version ${active.version}`}
            />
          </div>
          <div>
            <p className="text-xs font-medium text-gray-400 mb-1.5">Upgrade</p>
            <CodeBlock
              code={`helm upgrade my-release ${repoAlias}/${name} --version ${active.version}`}
            />
          </div>
          <div>
            <p className="text-xs font-medium text-gray-400 mb-1.5">
              Repository URL{" "}
              <span className="text-gray-600 font-normal">
                (for ArgoCD / FluxCD)
              </span>
            </p>
            <CodeBlock code={repoUrl} />
          </div>
          <div>
            <p className="text-xs font-medium text-gray-400 mb-1.5">
              Direct download
            </p>
            <CodeBlock
              code={`curl -LO "${base}/api/charts/${ownerUsername}/${name}/${active.version}/download"`}
            />
          </div>
        </div>
      ) : !active.values_yaml ? (
        <p className="text-sm text-gray-600 text-center py-8">
          No <code>values.yaml</code> in this version.
        </p>
      ) : (
        <div className="relative group max-h-[500px] overflow-y-auto rounded-lg border border-gray-800 bg-gray-950">
          <button
            onClick={() => navigator.clipboard.writeText(active.values_yaml!)}
            className="absolute top-2 right-2 p-1.5 rounded text-gray-600 hover:text-gray-300 hover:bg-gray-800 opacity-0 group-hover:opacity-100 transition-all z-10"
          >
            <Copy className="w-3.5 h-3.5" />
          </button>
          <pre className="px-4 py-3 text-xs font-mono text-gray-300 whitespace-pre">
            {active.values_yaml}
          </pre>
        </div>
      )}
    </div>
  );
}
