import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "@tauri-apps/plugin-os";
import App from "./App";

// Detect platform and add class to document for platform-specific styling
async function detectPlatform() {
  try {
    const os = await platform();
    document.documentElement.setAttribute("data-platform", os);
    // Also add class for easier CSS targeting
    document.documentElement.classList.add(`platform-${os}`);
  } catch {
    // Fallback: detect from user agent
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("win")) {
      document.documentElement.setAttribute("data-platform", "windows");
      document.documentElement.classList.add("platform-windows");
    } else if (ua.includes("mac")) {
      document.documentElement.setAttribute("data-platform", "macos");
      document.documentElement.classList.add("platform-macos");
    } else if (ua.includes("linux")) {
      document.documentElement.setAttribute("data-platform", "linux");
      document.documentElement.classList.add("platform-linux");
    }
  }
}

detectPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
