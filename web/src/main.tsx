import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isDesktop } from "./api/base";
import { App } from "./App";
import { DesktopApp } from "./desktop/DesktopApp";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");
createRoot(root).render(
  <StrictMode>
    {isDesktop() ? <DesktopApp /> : <App />}
  </StrictMode>,
);
