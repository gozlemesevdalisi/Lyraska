import { useCallback, useEffect, useRef, useState } from "react";
import {
  IDLE_STATUS,
  errorMessage,
  getAppInfo,
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
  /**
   * Son bilinen konum: ses motorunun düzenli bildiriminde (saniyede birkaç kez) ve sarmada
   * güncellenir. Her karede akıcı ilerleyen konum için [`useLivePosition`]: ekranın tamamı
   * her karede yeniden çizilmesin diye yalnızca onu gösteren bileşenler kullanır.
   */
  position: number;
  /** Tam şu anki konum (saat ölçümü; React çizimini beklemez). İşaretleme için. */
  positionNow: () => number;
  /** Ses çalınabilir mi? (Tarayıcı önizlemesinde hayır.) */
  available: boolean;
  /** Son komut hatası; bir sonraki başarılı komutta temizlenir. */
  error: string | null;
  openFile: () => Promise<void>;
  /**
   * Şarkıyı açar. `true`: açıldı; `false`: çekirdek açamadı (hata `error`'da); `undefined`:
   * başka bir komut sürdüğü için sıraya alındı, sonucu bilinmiyor.
   */
  openPath: (path: string) => Promise<boolean | undefined>;
  toggle: () => Promise<void>;
  stop: () => Promise<void>;
  /** Şarkıda verilen saniyeye atlar. Ekran beklemeden yeni konumu gösterir. */
  seek: (seconds: number) => Promise<void>;
  /**
   * Çekirdeğin durumunu yeniden sorar. Başka bir yer oturumu değiştirdiğinde (kör test çalan
   * şarkıyı kapatıp bitince geri açar) ekran güncellensin diye.
   */
  refresh: () => Promise<void>;
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
  /** Bir komut sürerken açılmak istenen şarkı (o komut bitince açılır). */
  const pendingOpen = useRef<string | null>(null);
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

  /**
   * Komutları sıraya koyar: aynı anda iki komut gönderilmez. Bir komut sürerken gelen komut
   * yok sayılır (art arda basışlar birikmez); yalnızca açılmak istenen şarkı bekletilir ve
   * komut bitince açılır (#52).
   */
  const run = useCallback(
    async (command: () => Promise<PlaybackStatus | void>): Promise<boolean | undefined> => {
      if (busy.current) return undefined;
      busy.current = true;
      // İlk komutun sonucu çağırana döner (bekletilen açılışlar kendi hatasını `error`'a yazar).
      let succeeded = true;
      try {
        let next: (() => Promise<PlaybackStatus | void>) | null = command;
        let first = true;
        while (next) {
          try {
            const result = await next();
            applyStatus(result ?? (await getPlaybackStatus()), true);
            setError(null);
          } catch (e) {
            if (first) succeeded = false;
            setError(errorMessage(e));
            applyStatus(await getPlaybackStatus().catch(() => IDLE_STATUS), true);
          }
          first = false;
          const waiting = pendingOpen.current;
          pendingOpen.current = null;
          next = waiting === null ? null : () => openTrack(waiting).then(() => undefined);
        }
      } finally {
        busy.current = false;
      }
      return succeeded;
    },
    [applyStatus],
  );

  const openPath = useCallback(
    (path: string) => {
      // Bir komut (ör. başka bir şarkının açılışı) sürüyorsa bu şarkı onun ardından açılır;
      // arada istenenlerden yalnızca sonuncusu.
      if (busy.current) {
        pendingOpen.current = path;
        return Promise.resolve(undefined);
      }
      return run(async () => {
        await openTrack(path);
      });
    },
    [run],
  );

  const openFile = useCallback(async () => {
    if (!available || busy.current) return;
    try {
      // Uzantı listesi program bilgisiyle gelir; henüz gelmediyse önce onu al ki pencere
      // süzgeçsiz açılmasın.
      const filters = extensions.length > 0 ? extensions : (await getAppInfo()).supportedExtensions;
      const path = await pickAudioFile(filters);
      if (path) await openPath(path);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [available, extensions, openPath]);

  const toggle = useCallback(() => run(togglePlayback).then(() => undefined), [run]);
  const stop = useCallback(() => run(stopPlayback).then(() => undefined), [run]);

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
    // Yanıt gelmeden yenisi gönderilmez: çekirdek meşgulken (şarkı açılırken kilit birkaç
    // saniye tutulabilir) sorgular birikip çekirdeğin iş parçacıklarını bekletmesin, eski
    // yanıt yenisinden sonra gelip konumu geri çekmesin.
    let inFlight = false;
    let cancelled = false;
    const timer = window.setInterval(() => {
      if (inFlight) return;
      inFlight = true;
      getPlaybackStatus()
        .then((next) => !cancelled && applyStatus(next, false))
        .catch(() => {
          /* Geçici hata: bir sonraki turda yeniden denenir. */
        })
        .finally(() => {
          inFlight = false;
        });
    }, POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [available, hasTrack, applyStatus]);

  const refresh = useCallback(async () => {
    if (!available) return;
    try {
      applyStatus(await getPlaybackStatus(), true);
    } catch {
      /* Geçici hata: bir sonraki komutta yeniden sorulur. */
    }
  }, [available, applyStatus]);

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
    refresh,
  };
}

/**
 * Çalarken her ekran karesinde akıcı ilerleyen konum (duraklatılmışken son bilinen konum).
 * Yalnızca konumu gösteren küçük bileşenler (süre, şarkı haritası şeridi, drop sayacı)
 * kullanır: yoksa her karede bütün ekran (çekmecedeki paneller, kütüphane listesi) yeniden
 * çizilirdi; büyük kütüphanede bu, karenin önemli bir kısmını yiyordu.
 */
export function useLivePosition(
  player: Pick<PlayerControls, "status" | "position" | "positionNow">,
) {
  const { status, position, positionNow } = player;
  const playing = status.state === "playing";
  const [live, setLive] = useState(position);
  // Sarma ve ses motorunun bildirimi bir sonraki kareyi beklemeden görünsün.
  const [known, setKnown] = useState(position);
  if (known !== position) {
    setKnown(position);
    setLive(position);
  }
  useEffect(() => {
    if (!playing || typeof window.requestAnimationFrame !== "function") return;
    let raf = 0;
    const tick = () => {
      setLive(positionNow());
      raf = window.requestAnimationFrame(tick);
    };
    raf = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(raf);
  }, [playing, positionNow]);
  // Duraklatılmışken ya da sarınca (bir sonraki kareden önce) bilinen konum.
  return playing ? live : position;
}
