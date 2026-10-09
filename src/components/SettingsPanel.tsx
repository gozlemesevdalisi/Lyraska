import { useState } from "react";
import { errorMessage, openLog, type AppInfo, type PlaybackStatus } from "../lib/backend";
import { signalPathText } from "../lib/format";
import type { PlaybackOptionsControls } from "../hooks/usePlaybackOptions";
import type { VisualSafeControls } from "../hooks/useVisualSafe";

export interface SettingsPanelProps {
  info: AppInfo;
  status: PlaybackStatus;
  available: boolean;
  visualSafe: VisualSafeControls;
  playback: PlaybackOptionsControls;
}

const SHORTCUTS: [string, string][] = [
  ["Boşluk", "Çal / duraklat"],
  ["← →", "5 saniye geri / ileri"],
  ["1 – 4", "Sahne seç"],
  ["Ctrl + O", "Dosya aç"],
  ["Esc", "Paneli kapat"],
];

/** Ayarlar: görsel güvenlik, sesin yolu, kısayollar, program bilgisi ve hata günlüğü. */
export function SettingsPanel({
  info,
  status,
  available,
  visualSafe,
  playback,
}: SettingsPanelProps) {
  const [logError, setLogError] = useState<string | null>(null);
  const signalPath = signalPathText(status);
  const notice = status.output?.notice ?? null;

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
        <h2 className="settings__title">Ses</h2>
        <button
          type="button"
          className={`settings__toggle${playback.options.normalize ? " is-on" : ""}`}
          aria-pressed={playback.options.normalize}
          onClick={() => void playback.setNormalize(!playback.options.normalize)}
        >
          <span className="settings__switch" aria-hidden />
          Ses yüksekliği eşitleme: {playback.options.normalize ? "Açık" : "Kapalı"}
        </button>
        <p className="settings__hint">
          Bütün şarkılar aynı yükseklikte çalar (EBU R128, −14 LUFS); yüksek kaydedilmiş şarkılar
          kısılır, sessiz kaydedilmişler bozulmadan yükseltilir. Ekolayzer de sesi kısmadan
          yükseltebilir: bas dediğinizde bas gerçekten artar. Diğer programlardan biraz kısık
          gelirse Windows sesini açın.
        </p>
        <button
          type="button"
          className={`settings__toggle${playback.options.bitPerfect ? " is-on" : ""}`}
          aria-pressed={playback.options.bitPerfect}
          onClick={() => void playback.setBitPerfect(!playback.options.bitPerfect)}
        >
          <span className="settings__switch" aria-hidden />
          Bit-perfect (özel mod): {playback.options.bitPerfect ? "Açık" : "Kapalı"}
        </button>
        <p className="settings__hint">
          Şarkı ses aygıtına hiç değiştirilmeden, kendi hızında ve çözünürlüğünde gider; aygıt
          yalnızca Lyraska'ya ayrılır. Açıkken ekolayzer, kulaklık düzeltmesi ve ses yüksekliği
          eşitleme çalışmaz, diğer programların sesi duyulmaz.
        </p>
        <p className="settings__warn">
          Dikkat: özel modda Windows ses düzeyi çoğu aygıtta etkisizdir ve ses tam yükseklikte
          gelebilir. Açmadan önce kulaklığı çıkarın ya da aygıtın kendi ses düğmesini kısın.
        </p>
        {notice && (
          <p className="settings__warn">
            Bit-perfect açılamadı: {notice}. Ses normal yoldan çalıyor.
          </p>
        )}
        {playback.error && <p className="settings__warn">{playback.error}</p>}
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
