import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isDesktop } from "./api/base";
import { App } from "./App";
import { installExternalLinks } from "./desktop/bridge";
import { DesktopApp } from "./desktop/DesktopApp";
import { initTitlebar } from "./desktop/titlebar";
import { ErrorBoundary } from "./log/ErrorBoundary";
import { installGlobalHandlers } from "./log/report";
import { initTheme } from "./theme/theme";
import "./styles/index.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");
installGlobalHandlers();
initTheme();
initTitlebar();
installExternalLinks(document);
createRoot(root).render(
  <StrictMode>
    <ErrorBoundary>{isDesktop() ? <DesktopApp /> : <App />}</ErrorBoundary>
  </StrictMode>,
);
