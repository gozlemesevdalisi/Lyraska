import { useEffect, useMemo, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { GLYPH_HEIGHT, textToColumns } from "../lib/dotFont";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";

export interface MarqueeProps {
  text: string;
  /** Ekranda aynı anda görünen sütun sayısı. */
  width: number;
  /** Bir sütun kayma süresi (ms). Eski teyplerdeki gibi adım adım kayar. */
  stepMs?: number;
  className?: string;
}

/** Metnin belirli bir kaydırma konumunda görünen sütunlarını döndürür. */
export function marqueeWindow(source: boolean[][], offset: number, width: number): boolean[][] {
  const blank = new Array<boolean>(GLYPH_HEIGHT).fill(false);
  // Metin sağdan girer, soldan çıkar; arada ekran genişliği kadar boşluk vardır.
  const cycle = source.length + width;
  const start = ((offset % cycle) + cycle) % cycle;
  const view: boolean[][] = [];
  for (let i = 0; i < width; i++) {
    const index = start + i - width;
    view.push(source[index] ?? blank);
  }
  return view;
}

/** Sağdan sola adım adım kayan nokta matris yazı. */
export function Marquee({ text, width, stepMs = 90, className }: MarqueeProps) {
  const source = useMemo(() => textToColumns(text), [text]);
  const reducedMotion = usePrefersReducedMotion();
  const [offset, setOffset] = useState(width);

  useEffect(() => {
    if (reducedMotion) return;
    const timer = window.setInterval(() => setOffset((o) => o + 1), stepMs);
    return () => window.clearInterval(timer);
  }, [reducedMotion, stepMs]);

  // Hareket azaltma tercihinde metnin başı sabit olarak gösterilir.
  const columns = reducedMotion
    ? marqueeWindow(source, width, width)
    : marqueeWindow(source, offset, width);

  return <DotMatrix columns={columns} label={text} className={className} />;
}
