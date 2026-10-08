/**
 * İşaretleme aracı: kullanıcının şarkı çalarken tuşla vurduğu beat ve drop anları.
 * Saf hesaplar (test edilebilir); kayıt ve doğruluk ölçümü çekirdekte.
 */

export type MarkKind = "beat" | "drop";

export interface Marks {
  /** Artan sırada (saniye). */
  beats: number[];
  drops: number[];
  /** Geri almak için ekleme sırası. */
  history: { kind: MarkKind; time: number }[];
}

export const EMPTY_MARKS: Marks = { beats: [], drops: [], history: [] };

/** Aynı türden bu kadar yakın iki basış tek sayılır (tuş titremesi, çift basış). */
export const BOUNCE_SECONDS = 0.08;

/** İşaret ekler; geçersiz zaman ya da çok yakın tekrar yok sayılır. */
export function addMark(marks: Marks, kind: MarkKind, time: number): Marks {
  if (!Number.isFinite(time) || time < 0) return marks;
  const list = kind === "beat" ? marks.beats : marks.drops;
  if (list.some((t) => Math.abs(t - time) < BOUNCE_SECONDS)) return marks;
  const index = list.findIndex((t) => t > time);
  const next = index < 0 ? [...list, time] : [...list.slice(0, index), time, ...list.slice(index)];
  return {
    beats: kind === "beat" ? next : marks.beats,
    drops: kind === "drop" ? next : marks.drops,
    history: [...marks.history, { kind, time }],
  };
}

/** Son eklenen işareti geri alır. */
export function undoMark(marks: Marks): Marks {
  const last = marks.history[marks.history.length - 1];
  if (!last) return marks;
  const remove = (list: number[]) => {
    const i = list.lastIndexOf(last.time);
    return i < 0 ? list : [...list.slice(0, i), ...list.slice(i + 1)];
  };
  return {
    beats: last.kind === "beat" ? remove(marks.beats) : marks.beats,
    drops: last.kind === "drop" ? remove(marks.drops) : marks.drops,
    history: marks.history.slice(0, -1),
  };
}

/** Kayıtlı işaretlerden başlangıç durumu. */
export function marksFrom(beats: number[], drops: number[]): Marks {
  const sorted = (list: number[]) => [...list].sort((a, b) => a - b);
  return { beats: sorted(beats), drops: sorted(drops), history: [] };
}

/** Son vuruşlardan tempo (BPM): son 8 aralığın ortancası. En az 4 vuruş gerekir. */
export function tapBpm(beats: number[]): number | null {
  const recent = beats.slice(-9);
  const intervals = recent
    .slice(1)
    .map((t, i) => t - recent[i]!)
    .filter((d) => d > 0.2 && d < 2)
    .sort((a, b) => a - b);
  if (intervals.length < 3) return null;
  const median = intervals[Math.floor(intervals.length / 2)]!;
  return Math.round(600 / median) / 10;
}

/**
 * Tuşa basıldığı andaki şarkı konumu. Olay işlenene kadar geçen süre (genelde
 * birkaç ms) düşülür: `eventTime` olayın zamanı, `now` şimdiki zaman (ms).
 */
export function positionAtEvent(positionNow: number, now: number, eventTime: number): number {
  const elapsed = Math.max(0, Math.min(0.25, (now - eventTime) / 1000));
  return Math.max(0, positionNow - elapsed);
}
