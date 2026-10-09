import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { installErrorReporting } from "./lib/errorReporting";
import "@fontsource-variable/inter/opsz.css";
import "./styles/global.css";

installErrorReporting();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {/* Son güvence: bölümlerin kendi hata sınırları var; buraya yalnızca ana ekranın hatası gelir. */}
    <ErrorBoundary
      name="Lyraska"
      fallback={() => (
        <div className="app-error" role="alert">
          <p>Lyraska bir hatayla karşılaştı. Hata günlüğe yazıldı.</p>
          <button type="button" className="glass-button" onClick={() => window.location.reload()}>
            Yeniden başlat
          </button>
        </div>
      )}
    >
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
