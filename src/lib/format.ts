import type { PlaybackStatus, TrackInfo } from "./backend";

/** Saniyeyi teyp ekranı biçiminde yazar: "03:45", bir saatten uzunsa "1:02:03". */
export function formatTime(seconds: number): string {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Şarkının ekranda görünen adı: "Sanatçı - Başlık", yoksa başlık, o da yoksa dosya adı. */
export function trackTitle(track: TrackInfo): string {
  if (track.title && track.artist) return `${track.artist} - ${track.title}`;
  return track.title ?? track.fileName;
}

const KHZ = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1 });

/** Teknik bilgi satırı: "FLAC · 44,1 kHz · Stereo". */
export function trackTechLine(track: TrackInfo): string {
  const channels =
    track.channels === 1 ? "Mono" : track.channels === 2 ? "Stereo" : `${track.channels} kanal`;
  return [track.codec.toUpperCase(), `${KHZ.format(track.sampleRate / 1000)} kHz`, channels].join(
    " · ",
  );
}

/**
 * Sinyal yolu: sesin şarkıdan hoparlöre nasıl gittiği. Şarkı açık değilse `null`.
 * Ör. "MP3 44,1 kHz → 48 kHz (yüksek kalite) → Hoparlörler (Realtek) · taşma koruması".
 */
export function signalPathText(status: PlaybackStatus): string | null {
  const { track, output } = status;
  if (!track || !output) return null;
  const source = `${track.codec.toUpperCase()} ${KHZ.format(track.sampleRate / 1000)} kHz`;
  const conversion = output.resampled
    ? `${KHZ.format(output.sampleRate / 1000)} kHz (yüksek kalite)`
    : "dönüştürmesiz";
  const parts = [source, conversion, output.deviceName || "varsayılan ses aygıtı"];
  const problems = status.underruns > 0 ? ` · takılma: ${status.underruns}` : "";
  return `${parts.join(" → ")} · taşma koruması${problems}`;
}

/** Tempo: "128 BPM", kesirliyse "105,5 BPM". */
export function formatBpm(bpm: number): string {
  return `${KHZ.format(Math.round(bpm * 10) / 10)} BPM`;
}

/** İlerleme oranı (0..1); süre bilinmiyorsa 0. */
export function progress(positionSecs: number, durationSecs: number | null): number {
  if (!durationSecs || durationSecs <= 0) return 0;
  return Math.min(1, Math.max(0, positionSecs / durationSecs));
}
