import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { initializeFaro, getWebInstrumentations } from "@grafana/faro-web-sdk";
import "./index.css";
import App from "./App.tsx";

// Initialize Grafana Faro if an endpoint is provided.
// Usually this points to our backend proxy which forwards to Grafana Alloy.
const faroEnabled = import.meta.env.VITE_FARO_URL || import.meta.env.VITE_OTEL_COLLECTOR_ENDPOINT;
if (faroEnabled) {
  initializeFaro({
    url: "/api/telemetry/faro",
    app: {
      name: "helm-hub-frontend",
      version: "1.0.0",
      environment: import.meta.env.MODE,
    },
    instrumentations: [
      ...getWebInstrumentations({
        captureConsole: true,
      }),
    ],
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
