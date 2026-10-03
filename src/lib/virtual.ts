/**
 * Uzun listelerde yalnızca görünen satırları çizmek için hesap
 * (binlerce şarkıda bile kaydırma akıcı kalsın).
 */
export interface VisibleRange {
  /** İlk çizilecek satır (dahil). */
  start: number;
  /** Son çizilecek satır (hariç). */
  end: number;
}

/** Kaydırma konumuna göre çizilecek satır aralığı; kenarlarda `overscan` satır pay bırakır. */
export function visibleRange(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  count: number,
  overscan = 6,
): VisibleRange {
  if (count <= 0 || rowHeight <= 0) return { start: 0, end: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + 1;
  return {
    start: Math.max(0, first - overscan),
    end: Math.min(count, first + visible + overscan),
  };
}

/** Bir satırı görünür alana getirmek için gereken kaydırma konumu (gerekmiyorsa `null`). */
export function scrollToRow(
  index: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
): number | null {
  const top = index * rowHeight;
  const bottom = top + rowHeight;
  if (top < scrollTop) return top;
  if (bottom > scrollTop + viewportHeight) return bottom - viewportHeight;
  return null;
}
