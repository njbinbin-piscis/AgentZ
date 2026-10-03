import React from "react";
import ReactDOM from "react-dom/client";
import "./monaco-setup";
import "./i18n";
import { initAppearanceTheme, initUiFontScale } from "./theme";
import { initGraphicsMode } from "./utils/graphicsMode";
import App from "./App";
import "./index.css";
import "./theme-light-fixes.css";

initAppearanceTheme();
initUiFontScale();
initGraphicsMode();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
