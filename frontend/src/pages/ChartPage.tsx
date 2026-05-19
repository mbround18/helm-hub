import { Link, useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Package } from "lucide-react";
import { chartsApi, type Chart } from "../lib/api";
import { ChartDetail } from "../components/ChartDetail";
import { Seo } from "../components/Seo";
import { useSettings } from "../hooks/useSettings";
import { AppLogo } from "../components/AppLogo";

export function ChartPage() {
  const { owner = "", name = "" } = useParams();
  const { app_name } = useSettings();

  const canonicalPath = `/charts/${owner}/${name}`;
  const canonical = `${window.location.origin}${canonicalPath}`;

  const { data: chart, isLoading } = useQuery({
    queryKey: ["public-chart", owner, name],
    queryFn: async (): Promise<Chart | undefined> => {
      const { data } = await chartsApi.listByOwner(owner);
      return data.find((item) => item.name === name);
    },
    enabled: !!owner && !!name,
  });

  const title = chart
    ? `${chart.name} by ${owner} | ${app_name}`
    : `${name || "Chart"} | ${app_name}`;
  const description =
    chart?.description ??
    (chart
      ? `Helm chart ${chart.name} by ${owner}`
      : `Public Helm chart page for ${owner}/${name}`);
  const robots = chart ? "index,follow" : "noindex,follow";

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100">
      <Seo
        title={title}
        description={description}
        canonical={canonical}
        ogType="article"
        robots={robots}
      />

      <div className="border-b border-gray-800 bg-gray-900/60 backdrop-blur">
        <div className="max-w-6xl mx-auto px-6 py-4 flex items-center justify-between gap-4">
          <Link
            to="/"
            className="flex items-center gap-3 text-violet-400 hover:text-violet-300 transition-colors"
          >
            <AppLogo size="sm" showName={false} />
            <span className="text-sm font-medium">{app_name}</span>
          </Link>
          <Link
            to="/"
            className="flex items-center gap-1.5 text-sm text-gray-400 hover:text-white transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            Back to explore
          </Link>
        </div>
      </div>

      {isLoading ? (
        <div className="max-w-6xl mx-auto px-6 py-12 text-gray-500 text-sm">
          Loading chart…
        </div>
      ) : chart ? (
        <ChartDetail
          name={chart.name}
          ownerUsername={owner}
          description={chart.description}
          keywords={chart.keywords}
          downloadCount={chart.download_count}
          shareUrl={canonical}
        />
      ) : (
        <div className="max-w-3xl mx-auto px-6 py-16 text-center">
          <Package className="w-12 h-12 text-gray-700 mx-auto mb-4" />
          <h1 className="text-2xl font-semibold text-white mb-2">
            Chart not found
          </h1>
          <p className="text-gray-500 text-sm">
            The chart {owner ? <code>{owner}/{name}</code> : name} isn’t public
            or doesn’t exist.
          </p>
        </div>
      )}
    </div>
  );
}
