import { describe, expect, it } from "vitest";
import { timelineLayout } from "./timeline";

describe("şarkı haritası şeridi", () => {
  it("zamanları şeridin genişliğine oranlar", () => {
    const layout = timelineLayout({
      durationSecs: 200,
      sections: [
        { start: 0, end: 50, energy: 0.4 },
        { start: 50, end: 210, energy: 1.4 },
      ],
      programDrops: [60, 300],
      markedDrops: [61, -1],
    })!;
    expect(layout.sections).toEqual([
      { x: 0, width: 0.25, energy: 0.4 },
      { x: 0.25, width: 0.75, energy: 1 },
    ]);
    expect(layout.programDrops).toEqual([0.3]);
    expect(layout.markedDrops).toEqual([0.305]);
  });

  it("süre yoksa çizilmez", () => {
    const empty = { sections: [], programDrops: [], markedDrops: [] };
    expect(timelineLayout({ durationSecs: 0, ...empty })).toBeNull();
    expect(timelineLayout({ durationSecs: Number.NaN, ...empty })).toBeNull();
  });
});
