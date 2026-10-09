import type { PlaybackStatus } from "../lib/backend";
import { coverGradient } from "../lib/cover";
import { titleScale, trackName } from "../lib/format";

/** Başlığın üstündeki küçük satır: çalma durumu. */
export function stateLabel(status: PlaybackStatus): string {
  if (!status.track) return "Hoş geldiniz";
  switch (status.state) {
    case "playing":
      return "Şimdi çalıyor";
    case "paused":
      // Durdur düğmesi şarkıyı başa sarıp duraklatır.
      return status.positionSecs < 0.05 ? "Durduruldu" : "Duraklatıldı";
    case "ended":
      return "Bitti";
    case "error":
      return "Çalınamadı";
    default:
      return "Hazır";
  }
}

export interface NowPlayingProps {
  status: PlaybackStatus;
  /** Kütüphanedeki albüm adı (biliniyorsa). */
  album: string | null;
  /** Çalma sırasındaki yer: "3 / 10". */
  queueLabel: string | null;
  /** Gösterilecek hata (komut ya da çalma hatası). */
  error: string | null;
  available: boolean;
  /** Şarkının içindeki kapak (`data:` adresi); yoksa özgün renk kapağı. */
  cover?: string | null;
}

/**
 * Sahnenin sol altındaki büyük başlık: kapak, şarkının adı ve sanatçısı. Şarkı yokken
 * karşılama ve müziği pencereye sürükleme daveti. Hata olursa başlığın altında
 * kehribar renkle yazılır (yanıp sönmez).
 */
export function NowPlaying({
  status,
  album,
  queueLabel,
  error,
  available,
  cover = null,
}: NowPlayingProps) {
  const { track } = status;
  const problem = error ?? status.error;
  const over = [stateLabel(status), queueLabel].filter(Boolean).join(" · ");
  const title = track ? trackName(track) : "Lyraska";
  const byline = track ? [track.artist, album].filter(Boolean).join(" — ") : null;

  return (
    <section className={`now${track ? " has-art" : ""}`} aria-label="Çalan şarkı">
      {track && (
        <div
          className={`now__art${cover ? " is-real" : ""}`}
          role="img"
          aria-label={cover ? `Kapak: ${album ?? title}` : "Kapak yok"}
          style={cover ? undefined : { background: coverGradient(album ?? track.artist ?? title) }}
        >
          {cover && <img src={cover} alt="" draggable={false} />}
        </div>
      )}
      <div className="now__text">
        <p className={`now__over${status.state === "playing" ? " is-playing" : ""}`}>
          <span className="now__dot" aria-hidden />
          {over}
        </p>
        <h1 className={`now__title now__title--${titleScale(title)}`}>{title}</h1>
        {track ? (
          byline && <p className="now__byline">{byline}</p>
        ) : (
          <p className="now__byline">Müzik klasörünüzü ya da şarkılarınızı pencereye sürükleyin.</p>
        )}
        {!available && <p className="now__hint">Ses çalmak için programı Windows'ta açın.</p>}
        {problem && (
          <p className="now__error" role="alert">
            {problem}
          </p>
        )}
      </div>
    </section>
  );
}
