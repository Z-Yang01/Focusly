import React from "react";
import ReactDOM from "react-dom/client";
import App from "@/app/App";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { ToastHost } from "@/components/ToastHost";
import "@/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ToastHost>
        <App />
      </ToastHost>
    </ErrorBoundary>
  </React.StrictMode>,
);
