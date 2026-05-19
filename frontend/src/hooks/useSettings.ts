import { useQuery } from "@tanstack/react-query";
import { settingsApi, type AppSettings } from "../lib/api";

const FALLBACK: AppSettings = {
  app_name: "Helm Hub",
  logo_url: "",
  signup_enabled: true,
  local_auth_enabled: true,
  github_auth_enabled: false,
  gitlab_auth_enabled: false,
};

export function useSettings() {
  const { data } = useQuery({
    queryKey: ["app-settings"],
    queryFn: () => settingsApi.get().then((r) => r.data),
    staleTime: 60_000,
  });
  return data ?? FALLBACK;
}
