import { useState } from "react";
import { errorMessage, openLog, type AppInfo, type PlaybackStatus } from "../lib/backend";
import { signalPathText } from "../lib/format";
import type { VisualSafeControls } from "../hooks/useVisualSafe";

export interface SettingsPanelProps {
  info: AppInfo;
  status: PlaybackStatus;
  available: boolean;
  visualSafe: VisualSafeControls;
}

const SHORTCUTS: [string, string][] = [
  ["Boşluk", "Çal / duraklat"],
  ["← →", "5 saniye geri / ileri"],
  ["1 – 4", "Sahne seç"],
  ["Ctrl + O", "Dosya aç"],
  ["Esc", "Paneli kapat"],
];

/** Ayarlar: görsel güvenlik, sesin yolu, kısayollar, program bilgisi ve hata günlüğü. */
export function SettingsPanel({ info, status, available, visualSafe }: SettingsPanelProps) {
  const [logError, setLogError] = useState<string | null>(null);
  const signalPath = signalPathText(status);

  return (
    <section className="settings" aria-label="Ayarlar">
      <div className="settings__group">
        <h2 className="settings__title">Görsel güvenlik</h2>
        <button
          type="button"
          className={`settings__toggle${visualSafe.safe ? " is-on" : ""}`}
          aria-pressed={visualSafe.safe}
          onClick={() => void visualSafe.setSafe(!visualSafe.safe)}
        >
          <span className="settings__switch" aria-hidden />
          Epilepsi güvenli modu: {visualSafe.safe ? "Açık" : "Kapalı"}
        </button>
        <p className="settings__hint">
          Açıkken görseller daha sakin olur: saniyede en fazla bir vuruş, parlaklık daha yavaş
          değişir. Kapalıyken de hiçbir sahne saniyede üçten fazla parlamaz.
        </p>
        {visualSafe.error && <p className="settings__warn">{visualSafe.error}</p>}
      </div>

      <div className="settings__group">
        <h2 className="settings__title">Sesin yolu</h2>
        <p className="settings__text">
          {signalPath ?? "Bir şarkı açılınca sesin dosyadan hoparlöre giden yolu burada görünür."}
        </p>
        {status.track && (
          <p className={status.underruns > 0 ? "settings__warn" : "settings__hint"}>
            Kesinti: {status.underruns}
          </p>
        )}
        <p className="settings__hint">
          Şarkı, ses aygıtının kendi hızına Lyraska'nın stüdyo kalitesindeki dönüştürücüsüyle
          çevrilir; Windows sese dokunmaz. Taşma koruması, yüksek kayıtlardaki tepelerin cızırtı
          yapmasını önler.
        </p>
      </div>

      <div className="settings__group">
        <h2 className="settings__title">Klavye</h2>
        <dl className="settings__keys">
          {SHORTCUTS.map(([key, action]) => (
            <div key={key}>
              <dt>
                <kbd>{key}</kbd>
              </dt>
              <dd>{action}</dd>
            </div>
          ))}
        </dl>
      </div>

      <div className="settings__group">
        <h2 className="settings__title">Program</h2>
        <p className="settings__text">
          {info.phase} · Sürüm {info.version}
        </p>
        <p className="settings__hint">Ses motoru: {info.audioEngine}</p>
        {available && (
          <button
            type="button"
            className="glass-button"
            title="Hata ve çökme günlüğünü dosya gezgininde gösterir. Hata kaydı açarken bu dosyayı ekleyin."
            onClick={() => {
              openLog()
                .then(() => setLogError(null))
                .catch((e) => setLogError(errorMessage(e)));
            }}
          >
            Hata günlüğü
          </button>
        )}
        {logError && <p className="settings__warn">{logError}</p>}
      </div>
    </section>
  );
}
