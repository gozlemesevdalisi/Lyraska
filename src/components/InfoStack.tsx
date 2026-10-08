import type { CSSProperties } from "react";
import type { PlaybackStatus, SongMap } from "../lib/backend";
import { coverGradient } from "../lib/cover";
import { formatBpm, formatLabel } from "../lib/format";
import { dropCountdown, meterLabel } from "../lib/songMap";

export interface UpNext {
  title: string;
  artist: string | null;
  album: string | null;
}

export interface InfoStackProps {
  status: PlaybackStatus;
  /** Ekranda gösterilen (akıcı) konum. */
  positionSecs: number;
  songMap: SongMap | null;
  /** Ekolayzer ya da kulaklık düzeltmesi sesi değiştiriyor mu? */
  soundShaped: boolean;
  safe: boolean;
  next: UpNext | null;
}

/**
 * Sahnenin sağ altındaki bilgi kartları: yaklaşan drop'un geri sayımı (Lyraska şarkıyı
 * önceden bildiği için), tempo ve ses biçimi, sıradaki şarkı.
 */
export function InfoStack({
  status,
  positionSecs,
  songMap,
  soundShaped,
  safe,
  next,
}: InfoStackProps) {
  const { track } = status;
  if (!track) return null;
  const countdown =
    songMap && status.state !== "ended" ? dropCountdown(songMap.drops, positionSecs) : null;
  const tempo = status.bpm
    ? [formatBpm(status.bpm), songMap ? meterLabel(songMap.meter) : null]
        .filter(Boolean)
        .join(" · ")
    : null;

  return (
    <aside className="info" aria-label="Şarkı bilgisi">
      {countdown && (
        <div className="countdown">
          <span
            className="countdown__ring"
            style={{ "--progress": countdown.progress } as CSSProperties}
            aria-hidden
          >
            {Math.ceil(countdown.seconds)}
          </span>
          <span className="countdown__text">
            <small>Drop yaklaşıyor</small>
            {Math.ceil(countdown.seconds)} saniye sonra
          </span>
        </div>
      )}
      <ul className="info__chips">
        {tempo && <li className="info-chip">{tempo}</li>}
        <li className="info-chip">{formatLabel(track, status.output)}</li>
        {soundShaped && <li className="info-chip is-on">EQ açık</li>}
        {safe && <li className="info-chip is-on">Güvenli mod</li>}
        {status.underruns > 0 && <li className="info-chip is-warn">Takılma: {status.underruns}</li>}
      </ul>
      {next && (
        <div className="up-next">
          <span
            className="up-next__cover"
            style={{ background: coverGradient(next.album ?? next.artist ?? next.title) }}
            aria-hidden
          />
          <span className="up-next__text">
            <small>Sıradaki</small>
            {[next.title, next.artist].filter(Boolean).join(" · ")}
          </span>
        </div>
      )}
    </aside>
  );
}
