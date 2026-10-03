/**
 * Reduced-graphics mode for software-rendered webviews (VMs, no GPU). Under
 * llvmpipe every frame of a blur or a decorative infinite animation is painted
 * on the CPU, so the `agentz-low-gfx` root class turns those off (see index.css).
 * Progress spinners keep spinning.
 *
 * Override with localStorage `agentz-graphics` = "low" | "full" (default auto).
 * WebKit masks the WebGL renderer string, so auto-detection asks the backend.
 */

import { invoke } from "@tauri-apps/api/core";

const STORAGE_KEY = "agentz-graphics";

function setLowGraphics(low: boolean): void {
  document.documentElement.classList.toggle("agentz-low-gfx", low);
}

export function initGraphicsMode(): void {
  const pref = localStorage.getItem(STORAGE_KEY);
  if (pref === "low" || pref === "full") {
    setLowGraphics(pref === "low");
    return;
  }
  invoke<boolean>("platform_software_rendering")
    .then(setLowGraphics)
    .catch(() => {});
}
