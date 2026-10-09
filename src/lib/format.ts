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
  const conversion = output.bitPerfect
    ? `bit-perfect (özel mod${output.bitDepth ? `, ${output.bitDepth} bit` : ""})`
    : output.resampled
      ? `${KHZ.format(output.sampleRate / 1000)} kHz (yüksek kalite)`
      : "dönüştürmesiz";
  const parts = [source, conversion, output.deviceName || "varsayılan ses aygıtı"];
  const leveling =
    output.normalizationDb !== null ? ` · eşitleme ${formatDb(output.normalizationDb)}` : "";
  // Bit-perfect'te ses işlenmez; taşma koruması yalnızca tam ölçeği aşan (bozuk) örneklerde çalışır.
  const guard = output.bitPerfect ? "" : " · taşma koruması";
  const problems = status.underruns > 0 ? ` · takılma: ${status.underruns}` : "";
  return `${parts.join(" → ")}${leveling}${guard}${problems}`;
}

/** Desibel: "−5,2 dB", "+1 dB", "0 dB" (Türkçe ondalık virgül, gerçek eksi işareti). */
export function formatDb(db: number): string {
  const rounded = Math.round(db * 10) / 10;
  if (rounded === 0) return "0 dB";
  const sign = rounded < 0 ? "−" : "+";
  return `${sign}${KHZ.format(Math.abs(rounded))} dB`;
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

/** Büyük başlıkta gösterilen şarkı adı: etiketteki başlık, yoksa dosya adı. */
export function trackName(track: TrackInfo): string {
  return track.title?.trim() || track.fileName;
}

/**
 * Büyük başlığın boyu: kısa adlar en büyük, uzun adlar küçülür; böylece ad iki satırı
 * geçmez ve sahneyi kapatmaz.
 */
export function titleScale(title: string): "xl" | "l" | "m" {
  const length = Array.from(title).length;
  if (length <= 14) return "xl";
  if (length <= 28) return "l";
  return "m";
}

/**
 * Ses biçimi etiketi: "FLAC 44,1 kHz"; şarkı aygıtın hızına çevriliyorsa
 * "FLAC 44,1 → 48 kHz".
 */
export function formatLabel(track: TrackInfo, output: PlaybackStatus["output"]): string {
  const codec = track.codec.toUpperCase();
  const source = KHZ.format(track.sampleRate / 1000);
  if (output?.bitPerfect) {
    return `${codec} ${source} kHz · bit-perfect`;
  }
  if (output?.resampled && output.sampleRate !== track.sampleRate) {
    return `${codec} ${source} → ${KHZ.format(output.sampleRate / 1000)} kHz`;
  }
  return `${codec} ${source} kHz`;
}

/** Yoldaki dosyanın uzantısız adı ("D:\Müzik\Firuze.flac" → "Firuze"). */
export function fileStem(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}
