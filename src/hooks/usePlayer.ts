import { useCallback, useEffect, useRef, useState } from "react";
import {
  IDLE_STATUS,
  errorMessage,
  getPlaybackStatus,
  isDesktop,
  onFileDrop,
  openTrack,
  pickAudioFile,
  seekPlayback,
  stopPlayback,
  togglePlayback,
  type PlaybackStatus,
} from "../lib/backend";

/** Durum sorgulama aralığı. Ekranda saniye gösterildiği için 4 kez/sn yeterli. */
const POLL_MS = 250;
/** Ok tuşlarıyla sarma adımı (saniye). */
export const SEEK_STEP_SECONDS = 5;

export interface PlayerControls {
  status: PlaybackStatus;
  /** Ses çalınabilir mi? (Tarayıcı önizlemesinde hayır.) */
  available: boolean;
  /** Bir komut sürüyor (ör. dosya açılıyor). */
  busy: boolean;
  /** Son komut hatası; bir sonraki başarılı komutta temizlenir. */
  error: string | null;
  openFile: () => Promise<void>;
  openPath: (path: string) => Promise<void>;
  toggle: () => Promise<void>;
  stop: () => Promise<void>;
  /** Şarkıda verilen saniyeye atlar. */
  seek: (seconds: number) => Promise<void>;
}

/**
 * Oynatıcıyı yöneten hook: komutları Rust çekirdeğine iletir, durumu düzenli
 * aralıklarla sorar, klavye kısayollarını ve sürükle-bırak ile dosya açmayı bağlar.
 *
 * Kısayollar: Boşluk = çal/duraklat, ←/→ = 5 sn geri/ileri, Ctrl+O = dosya aç.
 */
export function usePlayer(extensions: string[]): PlayerControls {
  const available = isDesktop();
  const [status, setStatus] = useState<PlaybackStatus>(IDLE_STATUS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busyRef = useRef(false);

  /** Komutları sıraya koyar: aynı anda iki komut gönderilmez. */
  const run = useCallback(async (command: () => Promise<PlaybackStatus | void>) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      const next = await command();
      setStatus(next ?? (await getPlaybackStatus()));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
      setStatus(await getPlaybackStatus().catch(() => IDLE_STATUS));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  const openPath = useCallback(
    (path: string) =>
      run(async () => {
        await openTrack(path);
      }),
    [run],
  );

  const openFile = useCallback(async () => {
    if (!available || busyRef.current) return;
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
    (seconds: number) => run(() => seekPlayback(Math.max(0, seconds))),
    [run],
  );

  // Ok tuşlarında güncel konuma göre sarabilmek için son durumu sakla.
  const statusRef = useRef(status);
  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  // Şarkı açıkken durumu düzenli sor (konum, şarkı sonu, aygıt hataları).
  const hasTrack = status.track !== null;
  useEffect(() => {
    if (!available || !hasTrack) return;
    const timer = window.setInterval(() => {
      getPlaybackStatus()
        .then(setStatus)
        .catch(() => {
          /* Geçici hata: bir sonraki turda yeniden denenir. */
        });
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [available, hasTrack]);

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
        void seek(statusRef.current.positionSecs + direction * SEEK_STEP_SECONDS);
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void openFile();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [available, toggle, openFile, seek]);

  // Pencereye bırakılan ilk dosyayı aç.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onFileDrop((paths) => {
      const first = paths[0];
      if (first) void openPath(first);
    })
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch(() => {
        /* Sürükle-bırak isteğe bağlı bir kolaylıktır. */
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [openPath]);

  return { status, available, busy, error, openFile, openPath, toggle, stop, seek };
}
