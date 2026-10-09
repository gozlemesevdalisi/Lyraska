import { Component, type ErrorInfo, type ReactNode } from "react";
import { logFrontendError } from "../lib/backend";
import { describeError } from "../lib/errorReporting";

export interface ErrorBoundaryProps {
  /** Bölümün adı (bildirimde ve hata günlüğünde; ör. "Ekolayzer"). */
  name: string;
  children: ReactNode;
  /** Hata olunca gösterilecek (verilmezse kısa bir bildirim ve "Yeniden dene"). */
  fallback?: (retry: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  failed: boolean;
}

/**
 * Hata sınırı: içindeki bir bileşen çizilirken hata verirse yalnızca o bölüm "açılamadı"
 * yazar; pencerenin geri kalanı (sahne, çalma, diğer paneller) çalışmaya devam eder. Hata,
 * bileşen yoluyla birlikte hata günlüğüne yazılır.
 *
 * React'ta hata sınırı yalnızca sınıf bileşeniyle yazılabilir (bunun için kanca yok);
 * projedeki tek sınıf bileşeni budur.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    const path = (info.componentStack ?? "").trim().split("\n").slice(0, 6).join("\n");
    const message = `${this.props.name} çizilemedi: ${describeError(error)}`;
    logFrontendError(path ? `${message}\nBileşenler:\n${path}` : message).catch(() => {
      /* Günlüğe yazılamazsa yapılacak bir şey yok. */
    });
  }

  private retry = () => this.setState({ failed: false });

  render() {
    if (!this.state.failed) return this.props.children;
    if (this.props.fallback) return this.props.fallback(this.retry);
    return (
      <div className="section-error" role="alert">
        <p className="section-error__text">
          {this.props.name} açılamadı. Hata, günlüğe yazıldı (Ayarlar &gt; Hata günlüğü).
        </p>
        <button type="button" className="glass-button" onClick={this.retry}>
          Yeniden dene
        </button>
      </div>
    );
  }
}
