/**
 * İşaretleme panelindeki şarkı haritası şeridinin yerleşimi (saf hesap).
 * Zamanlar 0..1 arası yatay konuma çevrilir.
 */

export interface TimelineInput {
  durationSecs: number;
  sections: { start: number; end: number; energy: number }[];
  programDrops: number[];
  markedDrops: number[];
}

export interface TimelineLayout {
  sections: { x: number; width: number; energy: number }[];
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
      })),
    programDrops: input.programDrops.filter(inside).map(x),
    markedDrops: input.markedDrops.filter(inside).map(x),
  };
}
