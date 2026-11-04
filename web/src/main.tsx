import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isDesktop } from "./api/base";
import { App } from "./App";
import { DesktopApp } from "./desktop/DesktopApp";
import { initTheme } from "./theme/theme";
import "./styles/index.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");
initTheme();
createRoot(root).render(
  <StrictMode>
    {isDesktop() ? <DesktopApp /> : <App />}
  </StrictMode>,
);
