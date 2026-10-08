import { useCallback, useEffect, useRef, useState } from "react";
import {
  IDLE_STATUS,
  errorMessage,
  getPlaybackStatus,
  isDesktop,
  openTrack,
  pickAudioFile,
  seekPlayback,
  stopPlayback,
  togglePlayback,
  type PlaybackStatus,
} from "../lib/backend";
import { positionAt, reanchor, type ClockAnchor } from "../lib/clock";

/** Durum sorgulama aralığı. Ekrandaki süre arada saatle akıcı ilerler. */
const POLL_MS = 250;
/** Ok tuşlarıyla sarma adımı (saniye). */
export const SEEK_STEP_SECONDS = 5;

export interface PlayerControls {
  status: PlaybackStatus;
  /** Ekranda gösterilecek konum: her karede akıcı ilerler, sarmada anında değişir. */
  position: number;
  /** Tam şu anki konum (saat ölçümü; React çizimini beklemez). İşaretleme için. */
  positionNow: () => number;
  /** Ses çalınabilir mi? (Tarayıcı önizlemesinde hayır.) */
  available: boolean;
  /** Son komut hatası; bir sonraki başarılı komutta temizlenir. */
  error: string | null;
  openFile: () => Promise<void>;
  openPath: (path: string) => Promise<void>;
  toggle: () => Promise<void>;
  stop: () => Promise<void>;
  /** Şarkıda verilen saniyeye atlar. Ekran beklemeden yeni konumu gösterir. */
  seek: (seconds: number) => Promise<void>;
}

const now = () => performance.now();

/**
 * Oynatıcıyı yöneten hook: komutları Rust çekirdeğine iletir, durumu düzenli
 * aralıklarla sorar, konumu akıcı bir saatle gösterir ve klavye kısayollarını bağlar.
 * (Sürükle-bırak ana ekranda ele alınır: şarkılar sıraya, klasörler kütüphaneye.)
 *
 * Kısayollar: Boşluk = çal/duraklat, ←/→ = 5 sn geri/ileri, Ctrl+O = dosya aç.
 */
export interface PlayerOptions {
  /** Şarkı sonuna kadar çalınınca bir kez çağrılır (sıradaki şarkıya geçmek için). */
  onEnded?: (status: PlaybackStatus) => void;
}

export function usePlayer(extensions: string[], options: PlayerOptions = {}): PlayerControls {
  const available = isDesktop();
  const [status, setStatus] = useState<PlaybackStatus>(IDLE_STATUS);
  const [position, setPosition] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const statusRef = useRef(status);
  const anchor = useRef<ClockAnchor | null>(null);
  const busy = useRef(false);
  const seekInFlight = useRef(false);
  const pendingSeek = useRef<number | null>(null);
  const onEnded = useRef(options.onEnded);
  useEffect(() => {
    onEnded.current = options.onEnded;
  }, [options.onEnded]);

  /** Şu anki tahmini konum (saniye). */
  const positionNow = useCallback(() => {
    const current = statusRef.current;
    if (!anchor.current) return current.positionSecs;
    return positionAt(anchor.current, now(), current.track?.durationSecs ?? null);
  }, []);

  /**
   * Ses motorundan gelen durumu uygular. `exact` ise konum olduğu gibi alınır
   * (komut cevapları); değilse küçük sapmalar yumuşakça düzeltilir (düzenli sorgular).
   */
  const applyStatus = useCallback((next: PlaybackStatus, exact: boolean) => {
    const previous = statusRef.current;
    statusRef.current = next;
    setStatus(next);
    if (
      previous.state !== "ended" &&
      next.state === "ended" &&
      previous.track?.path === next.track?.path
    ) {
      onEnded.current?.(next);
    }

    // Sarma sürerken gelen eski konumlar ekranı geri çekmesin.
    if (seekInFlight.current && !exact) return;

    const running = next.state === "playing";
    const duration = next.track?.durationSecs ?? null;
    const trackChanged = previous.track?.path !== next.track?.path;
    anchor.current =
      exact || trackChanged
        ? { position: next.positionSecs, at: now(), running }
        : reanchor(anchor.current, next.positionSecs, running, now(), duration);
    setPosition(positionAt(anchor.current, now(), duration));
  }, []);

  /** Komutları sıraya koyar: aynı anda iki komut gönderilmez. */
  const run = useCallback(
    async (command: () => Promise<PlaybackStatus | void>) => {
      if (busy.current) return;
      busy.current = true;
      try {
        const next = await command();
        applyStatus(next ?? (await getPlaybackStatus()), true);
        setError(null);
      } catch (e) {
        setError(errorMessage(e));
        applyStatus(await getPlaybackStatus().catch(() => IDLE_STATUS), true);
      } finally {
        busy.current = false;
      }
    },
    [applyStatus],
  );

  const openPath = useCallback(
    (path: string) =>
      run(async () => {
        await openTrack(path);
      }),
    [run],
  );

  const openFile = useCallback(async () => {
    if (!available || busy.current) return;
    try {
      const path = await pickAudioFile(extensions);
      if (path) await openPath(path);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [available, extensions, openPath]);

  const toggle = useCallback(() => run(togglePlayback), [run]);
  const stop = useCallback(() => run(stopPlayback), [run]);

  const seek = useCallback(
    async (seconds: number) => {
      const current = statusRef.current;
      if (!current.track) return;
      const duration = current.track.durationSecs ?? Infinity;
      const target = Math.min(Math.max(0, seconds), duration);

      // Ekranı hemen yeni konuma taşı; ses motoru arkadan yetişir.
      anchor.current = { position: target, at: now(), running: current.state === "playing" };
      setPosition(target);

      // Bir sarma sürüyorsa yalnızca en son hedefi hatırla (art arda basışlar birikir).
      if (seekInFlight.current) {
        pendingSeek.current = target;
        return;
      }
      seekInFlight.current = true;
      try {
        let next = target;
        for (;;) {
          const result = await seekPlayback(next);
          if (pendingSeek.current === null) {
            seekInFlight.current = false;
            applyStatus(result, true);
            break;
          }
          next = pendingSeek.current;
          pendingSeek.current = null;
        }
        setError(null);
      } catch (e) {
        seekInFlight.current = false;
        pendingSeek.current = null;
        setError(errorMessage(e));
        applyStatus(await getPlaybackStatus().catch(() => IDLE_STATUS), true);
      }
    },
    [applyStatus],
  );

  // Şarkı açıkken durumu düzenli sor (konum, şarkı sonu, aygıt hataları).
  const hasTrack = status.track !== null;
  useEffect(() => {
    if (!available || !hasTrack) return;
    const timer = window.setInterval(() => {
      getPlaybackStatus()
        .then((next) => applyStatus(next, false))
        .catch(() => {
          /* Geçici hata: bir sonraki turda yeniden denenir. */
        });
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [available, hasTrack, applyStatus]);

  // Çalarken konumu her ekran karesinde ilerlet.
  const playing = status.state === "playing";
  useEffect(() => {
    if (!playing || typeof window.requestAnimationFrame !== "function") return;
    let raf = 0;
    const tick = () => {
      setPosition(positionNow());
      raf = window.requestAnimationFrame(tick);
    };
    raf = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(raf);
  }, [playing, positionNow]);

  // Klavye kısayolları
  useEffect(() => {
    if (!available) return;
    const onKey = (event: KeyboardEvent) => {
      // Odak bir düğmedeyse Boşluk o düğmeye basar; çift komut gönderme.
      const target = event.target instanceof Element ? event.target : null;
      const interactive = target?.closest("button, input, select, textarea, [contenteditable]");
      if (event.code === "Space" && !interactive && !event.repeat) {
        event.preventDefault();
        void toggle();
      } else if (
        (event.key === "ArrowLeft" || event.key === "ArrowRight") &&
        !interactive &&
        statusRef.current.track
      ) {
        event.preventDefault();
        const direction = event.key === "ArrowRight" ? 1 : -1;
        // Ekranda görülen konumdan say: art arda basışlar birikir.
        void seek(positionNow() + direction * SEEK_STEP_SECONDS);
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void openFile();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [available, toggle, openFile, seek, positionNow]);

  return {
    status,
    position,
    positionNow,
    available,
    error,
    openFile,
    openPath,
    toggle,
    stop,
    seek,
  };
}
