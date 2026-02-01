import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "@tauri-apps/plugin-os";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import DataCollection from "./DataCollection";

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

// Determine which component to render based on window label
async function getWindowComponent(): Promise<React.ComponentType> {
  try {
    const window = getCurrentWindow();
    const label = window.label;
    
    if (label === "data-collection") {
      return DataCollection;
    }
    
    // Default to main App
    return App;
  } catch {
    return App;
  }
}

async function init() {
  await detectPlatform();
  const Component = await getWindowComponent();
  
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <Component />
    </React.StrictMode>,
  );
}

init();
