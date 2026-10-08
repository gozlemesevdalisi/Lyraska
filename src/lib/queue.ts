/**
 * Çalma sırası: kütüphaneden bir şarkı çalınınca, o anki liste sıra olur.
 * Şarkı bitince ya da "sonraki"ye basılınca sıradaki çalar.
 */
export interface Queue {
  paths: string[];
  index: number;
}

export const EMPTY_QUEUE: Queue = { paths: [], index: -1 };

/** Listeden bir şarkı seçilince yeni sıra. */
export function queueFrom(paths: string[], path: string): Queue {
  const index = paths.indexOf(path);
  return index < 0 ? { paths: [path], index: 0 } : { paths, index };
}

/** Sonraki şarkı; sıra bittiyse `null`. */
export function nextInQueue(queue: Queue): Queue | null {
  return queue.index + 1 < queue.paths.length ? { ...queue, index: queue.index + 1 } : null;
}

/** Önceki şarkı; başta ise `null`. */
export function previousInQueue(queue: Queue): Queue | null {
  return queue.index > 0 ? { ...queue, index: queue.index - 1 } : null;
}

export function currentPath(queue: Queue): string | null {
  return queue.paths[queue.index] ?? null;
}

/**
 * Boşluksuz geçiş: çekirdek sıradaki şarkıya kendiliğinden geçer. Çalan şarkı
 * sıradakiyse sıra bir ilerlemiş sayılır; değilse sıra aynen döner.
 */
export function followQueue(queue: Queue, playingPath: string | null): Queue {
  const next = nextInQueue(queue);
  return next && playingPath !== null && currentPath(next) === playingPath ? next : queue;
}

/** Çekirdeğe önceden bildirilecek sıradaki şarkı (sıra etkin değilse `null`). */
export function upcomingPath(queue: Queue, playingPath: string | null): string | null {
  if (playingPath === null || currentPath(queue) !== playingPath) return null;
  const next = nextInQueue(queue);
  return next ? currentPath(next) : null;
}

/** "Önceki" düğmesi: şarkının ilk 3 saniyesinden sonra basılırsa şarkı başa sarılır. */
export const RESTART_THRESHOLD_SECONDS = 3;
