import { Package } from "lucide-react";
import { useSettings } from "../hooks/useSettings";

interface AppLogoProps {
  size?: "sm" | "md" | "lg";
  showName?: boolean;
  className?: string;
}

const sizes = {
  sm: { icon: "w-4 h-4", text: "text-base" },
  md: { icon: "w-5 h-5", text: "text-lg" },
  lg: { icon: "w-8 h-8", text: "text-3xl" },
};

export function AppLogo({
  size = "md",
  showName = true,
  className = "",
}: AppLogoProps) {
  const { app_name, logo_url } = useSettings();
  const s = sizes[size];

  return (
    <span className={`flex items-center gap-2 ${className}`}>
      {logo_url ? (
        <img
          src={logo_url}
          alt={app_name}
          className={`${s.icon} object-contain`}
        />
      ) : (
        <Package className={s.icon} />
      )}
      {showName && (
        <span className={`font-semibold ${s.text}`}>{app_name}</span>
      )}
    </span>
  );
}
