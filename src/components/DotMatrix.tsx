import { memo } from "react";
import { GLYPH_HEIGHT } from "../lib/dotFont";

export interface DotMatrixProps {
  /** Sütunlar; her sütun yukarıdan aşağıya "yanıyor mu" bilgisi taşır. */
  columns: boolean[][];
  /** Satır sayısı (varsayılan: yazı tipi yüksekliği). */
  rows?: number;
  /** Ekran okuyucular için metin karşılığı. */
  label?: string;
  className?: string;
}

/** Noktalar arası mesafe (SVG birimi). Nokta yarıçapı bunun yaklaşık %38'i. */
const PITCH = 10;
const RADIUS = 3.8;

/**
 * Yanan ve sönük ("hayalet") noktalardan oluşan bir nokta matris ekran.
 * Sönük noktalar, gerçek VFD/LCD ekranlardaki gibi her zaman hafifçe görünür.
 */
export const DotMatrix = memo(function DotMatrix({
  columns,
  rows = GLYPH_HEIGHT,
  label,
  className,
}: DotMatrixProps) {
  const width = Math.max(columns.length, 1) * PITCH;
  const height = rows * PITCH;
  const lit: string[] = [];
  const ghost: string[] = [];

  columns.forEach((column, x) => {
    for (let y = 0; y < rows; y++) {
      const cx = x * PITCH + PITCH / 2;
      const cy = y * PITCH + PITCH / 2;
      // Bütün noktaları tek bir <path> içinde çizmek, binlerce <circle>
      // öğesine göre çok daha hızlıdır.
      const d = `M${cx - RADIUS},${cy}a${RADIUS},${RADIUS} 0 1,0 ${RADIUS * 2},0a${RADIUS},${RADIUS} 0 1,0 ${-RADIUS * 2},0`;
      (column[y] ? lit : ghost).push(d);
    }
  });

  return (
    <svg
      className={["dot-matrix", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${width} ${height}`}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      preserveAspectRatio="xMidYMid meet"
    >
      <path className="dot-matrix__ghost" d={ghost.join("")} />
      <path className="dot-matrix__lit" d={lit.join("")} data-lit-count={lit.length} />
    </svg>
  );
});
