import { useCallback, useEffect, useState } from "react";
import { setNextTrack, type PlaybackStatus } from "../lib/backend";
import {
  EMPTY_QUEUE,
  RESTART_THRESHOLD_SECONDS,
  currentPath,
  followQueue,
  nextInQueue,
  previousInQueue,
  queueFrom,
  upcomingPath,
  type Queue,
} from "../lib/queue";
import { usePlayer, type PlayerControls } from "./usePlayer";

export interface PlaybackControls {
  player: PlayerControls;
  /** Listeyi sıra yapıp verilen şarkıdan çalmaya başlar. */
  playList: (paths: string[], start: string) => void;
  next: () => void;
  /** Önceki şarkı; şarkının ortasındaysa önce başa sarar. */
  previous: () => void;
  canNext: boolean;
  /** Çalma sırasındaki yer ("3 / 10"); sıra yoksa ya da tek şarkıysa `null`. */
  queueLabel: string | null;
  /** Sıradaki şarkının yolu (boşluksuz geçiş için çekirdeğe bildirilen). */
  upcoming: string | null;
}

/**
 * Oynatıcı ve çalma sırası: kütüphaneden (ya da pencereye bırakılarak) çalınan liste
 * sıra olur. Sıradaki şarkı çekirdeğe önceden bildirilir, şarkı bitince ses kesilmeden
 * ona geçilir; boşluksuz geçiş olamazsa şarkı sonunda sıradaki açılır.
 */
export function usePlayback(extensions: string[]): PlaybackControls {
  const [queue, setQueue] = useState<Queue>(EMPTY_QUEUE);

  // Sıradaki şarkıyı çalmak için oynatıcıya ihtiyaç var; oynatıcı da şarkı bitince
  // sırayı soruyor. Döngüyü kırmak için açma işlevi sonradan bağlanır.
  const [openPathRef] = useState<{ current: (path: string) => Promise<void> }>(() => ({
    current: async () => {},
  }));
  const playQueue = useCallback(
    (next: Queue) => {
      const path = currentPath(next);
      if (!path) return;
      setQueue(next);
      void openPathRef.current(path);
    },
    [openPathRef],
  );
  const onEnded = useCallback(
    (ended: PlaybackStatus) => {
      // Yalnızca sıradan çalınan şarkı bittiyse sonrakine geç (boşluksuz geçiş
      // olamadıysa: ör. kanal sayısı farklı ya da şarkı açılamadı).
      const current = followQueue(queue, ended.track?.path ?? null);
      if (currentPath(current) !== ended.track?.path) return;
      const next = nextInQueue(current);
      if (next) playQueue(next);
    },
    [queue, playQueue],
  );

  const player = usePlayer(extensions, { onEnded });
  const { status } = player;
  const statusPath = status.track?.path ?? null;
  // Boşluksuz geçişte çekirdek sıradakine kendisi geçer: sıra buna göre ilerlemiş sayılır.
  const activeQueue = followQueue(queue, statusPath);
  // İlerleyen sıra saklanır: yoksa ikinci geçişte sıra eski yerinden bakar, çalan
  // şarkıyı tanımaz ve çalma o şarkının sonunda durur. (Çizim sırasında güncellenir:
  // sıra ilerlemediyse `followQueue` aynı nesneyi döndürür, döngü olmaz.)
  if (activeQueue !== queue) setQueue(activeQueue);

  // Sıradaki şarkı çekirdeğe önceden bildirilir: şarkı bitince ses kesilmeden ona geçer.
  const upcoming = upcomingPath(activeQueue, statusPath);
  useEffect(() => {
    setNextTrack(upcoming).catch(() => {
      /* Bildirilemezse şarkı sonunda normal geçiş yapılır. */
    });
  }, [upcoming]);
  useEffect(() => {
    openPathRef.current = player.openPath;
  }, [openPathRef, player.openPath]);

  // Sıra yalnızca çalan şarkı sıradaysa geçerlidir ("Dosya aç" ile tek şarkı açılınca değil).
  const queueActive = status.track !== null && currentPath(activeQueue) === status.track.path;
  const canNext = queueActive && nextInQueue(activeQueue) !== null;

  const playList = useCallback(
    (paths: string[], start: string) => playQueue(queueFrom(paths, start)),
    [playQueue],
  );
  const next = () => {
    const target = queueActive ? nextInQueue(activeQueue) : null;
    if (target) playQueue(target);
  };
  const previous = () => {
    const target = queueActive ? previousInQueue(activeQueue) : null;
    // Şarkının ortasındaysa önce başa sar (alışılmış davranış).
    if (player.position > RESTART_THRESHOLD_SECONDS || !target) void player.seek(0);
    else playQueue(target);
  };

  return {
    player,
    playList,
    next,
    previous,
    canNext,
    queueLabel:
      queueActive && activeQueue.paths.length > 1
        ? `${activeQueue.index + 1} / ${activeQueue.paths.length}`
        : null,
    upcoming,
  };
}
