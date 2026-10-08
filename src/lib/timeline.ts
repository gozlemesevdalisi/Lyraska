/**
 * Şarkı haritası şeridinin yerleşimi (saf hesap): alttaki çalma şeridi ve işaretleme
 * paneli kullanır. Zamanlar 0..1 arası yatay konuma çevrilir.
 */
import { sectionTheme } from "./songMap";

export interface TimelineInput {
  durationSecs: number;
  /** `label`: benzer bölümler aynı etiketi alır (renk teması); yoksa ilk tema. */
  sections: { start: number; end: number; energy: number; label?: number }[];
  programDrops: number[];
  markedDrops: number[];
}

export interface TimelineLayout {
  sections: { x: number; width: number; energy: number; theme: number }[];
  programDrops: number[];
  markedDrops: number[];
}

/** Süre bilinmiyorsa ya da sıfırsa `null`. Şarkı dışındaki zamanlar atılır. */
export function timelineLayout(input: TimelineInput): TimelineLayout | null {
  const { durationSecs } = input;
  if (!Number.isFinite(durationSecs) || durationSecs <= 0) return null;
  const x = (t: number) => Math.min(1, Math.max(0, t / durationSecs));
  const inside = (t: number) => Number.isFinite(t) && t >= 0 && t <= durationSecs;
  return {
    sections: input.sections
      .filter((s) => s.end > s.start && s.start < durationSecs)
      .map((s) => ({
        x: x(s.start),
        width: x(Math.min(s.end, durationSecs)) - x(s.start),
        energy: Math.min(1, Math.max(0, s.energy)),
        theme: sectionTheme(s.label ?? 0),
      })),
    programDrops: input.programDrops.filter(inside).map(x),
    markedDrops: input.markedDrops.filter(inside).map(x),
  };
}
