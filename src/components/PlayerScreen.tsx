import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { DropOverlay } from "./DropOverlay";
import { EqualizerPanel } from "./EqualizerPanel";
import { InfoStack, type UpNext } from "./InfoStack";
import { LibraryPanel } from "./LibraryPanel";
import { MarkerPanel } from "./MarkerPanel";
import { NowPlaying } from "./NowPlaying";
import { SeekBar } from "./SeekBar";
import { SettingsPanel } from "./SettingsPanel";
import { SpectrumDemo } from "./SpectrumDemo";
import { SpectrumView } from "./SpectrumView";
import { HighwayScene } from "./HighwayScene";
import { SkyScene } from "./SkyScene";
import { SyncPanel } from "./SyncPanel";
import { VuScene } from "./VuScene";
import {
  CloseIcon,
  EjectIcon,
  LibraryIcon,
  LyraMark,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PreviousIcon,
  SettingsIcon,
  SlidersIcon,
  StopIcon,
} from "./icons";
import {
  BROWSER_FALLBACK,
  getAppInfo,
  logFrontendError,
  onDragDrop,
  errorMessage,
  setNextTrack,
  type AppInfo,
  type LibraryTrack,
  type PlaybackStatus,
} from "../lib/backend";
import { DROP_NOTICE_MS, DROP_PROBLEM_MS, dropSummary, type DropNotice } from "../lib/drop";
import { describeError } from "../lib/errorReporting";
import { SCENES, loadScene, saveScene, sceneForKey, type Scene } from "../lib/scene";
import { fileStem, formatTime, trackTitle } from "../lib/format";
import { timelineLayout } from "../lib/timeline";
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
import { useEqualizer } from "../hooks/useEqualizer";
import { useHeadphone } from "../hooks/useHeadphone";
import { useIdle } from "../hooks/useIdle";
import { useSongMap } from "../hooks/useSongMap";
import { useSync } from "../hooks/useSync";
import { useVisualSafe } from "../hooks/useVisualSafe";
import { useMarker } from "../hooks/useMarker";
import { useLibrary } from "../hooks/useLibrary";
import { usePlayer } from "../hooks/usePlayer";

type Panel = "library" | "eq" | "marker" | "sync" | "settings";

const PANELS: [Panel, string][] = [
  ["library", "Kütüphane"],
  ["eq", "Ekolayzer"],
  ["marker", "İşaretle"],
  ["sync", "Senkron"],
  ["settings", "Ayarlar"],
];

/** Müzik çalarken fare bu kadar kıpırdamazsa düğmeler çekilir, yalnızca sahne kalır. */
const IDLE_MS = 4000;
const SPECTRUM_BANDS = 16;
const SPECTRUM_ROWS = 10;

/** Kütüphanede yolu verilen şarkı (arama sonucunda varsa). */
function libraryTrack(tracks: LibraryTrack[], path: string | null): LibraryTrack | null {
  return path === null ? null : (tracks.find((t) => t.path === path) ?? null);
}

/**
 * Ana ekran: seçilen görsel sahne tüm pencereyi kaplar; şarkının adı, şarkı haritası
 * ve düğmeler sahnenin üstünde cam katmanlarda durur. Kütüphane, ekolayzer ve
 * diğer paneller sağdan açılan çekmecededir. Müzik çalarken fare bir süre
 * kıpırdamazsa düğmeler çekilir, yalnızca sahne kalır.
 */
export function PlayerScreen() {
  const [info, setInfo] = useState<AppInfo>(BROWSER_FALLBACK);
  const library = useLibrary();
  const equalizer = useEqualizer();
  const headphone = useHeadphone();
  const visualSafe = useVisualSafe();
  // Ekolayzer ya da kulaklık düzeltmesi sesi değiştiriyorsa ekolayzer ışığı yanar.
  const soundShaped = equalizer.active || headphone.active;
  const [panel, setPanel] = useState<Panel>("library");
  // Çekmece açılışta açıktır: henüz bir şey çalmıyor, kütüphane ilk iştir.
  const [drawerOpen, setDrawerOpen] = useState(true);
  const [scene, setScene] = useState<Scene>(loadScene);
  const chooseScene = useCallback((next: Scene) => {
    setScene(next);
    saveScene(next);
  }, []);
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

  const player = usePlayer(info.supportedExtensions, { onEnded });
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

  const playing = status.state === "playing";
  const songMap = useSongMap(statusPath);
  const marker = useMarker(statusPath, playing, player.positionNow, songMap);
  const sync = useSync(
    player.openPath,
    player.positionNow,
    status.state === "ended" ? statusPath : null,
  );

  const selectPanel = (id: Panel) => {
    setPanel(id);
    // Sekmeden çıkınca işaretleme biter: Boşluk yine çal/duraklat olur.
    if (id !== "marker") marker.setRecording(false);
    // Senkron ölçümü de sekmeden çıkınca biter.
    if (id !== "sync") sync.finishCalibration();
  };
  const { setRecording } = marker;
  const { finishCalibration } = sync;
  const closeDrawer = useCallback(() => {
    setDrawerOpen(false);
    setRecording(false);
    finishCalibration();
  }, [setRecording, finishCalibration]);
  /** Üst çubuktaki düğmeler: paneli açar; zaten açıksa çekmeceyi kapatır. */
  const togglePanel = (id: Panel) => {
    if (drawerOpen && panel === id) {
      closeDrawer();
    } else {
      selectPanel(id);
      setDrawerOpen(true);
    }
  };

  const playFromLibrary = (track: LibraryTrack, list: LibraryTrack[]) => {
    playQueue(
      queueFrom(
        list.map((t) => t.path),
        track.path,
      ),
    );
    // Çalmaya başlayınca sahne ortaya çıksın; kütüphane düğmesiyle geri açılır.
    closeDrawer();
  };
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

  // Pencereye bırakılanlar: şarkılar ve klasörler kütüphaneye eklenir, şarkılar bırakılış
  // sırasıyla çalınır. Dosya mı klasör mü olduğuna çekirdek diskte bakar. Sonuç (sorunlar
  // dahil) hangi panel açık olursa olsun kısa bir bildirimle gösterilir.
  const { addDropped } = library;
  const [dragging, setDragging] = useState(false);
  const [dropNotice, setDropNotice] = useState<DropNotice | null>(null);
  const handleDropped = useCallback(
    async (paths: string[]) => {
      let notice: DropNotice;
      try {
        const outcome = await addDropped(paths);
        const first = outcome.tracks[0];
        if (first) {
          playQueue(queueFrom(outcome.tracks, first));
          closeDrawer();
        }
        notice = dropSummary(outcome);
      } catch (e) {
        notice = { text: errorMessage(e), problem: true };
      }
      setDropNotice(notice);
    },
    [addDropped, playQueue, closeDrawer],
  );
  useEffect(() => {
    if (!dropNotice) return;
    const ms = dropNotice.problem ? DROP_PROBLEM_MS : DROP_NOTICE_MS;
    const timer = window.setTimeout(() => setDropNotice(null), ms);
    return () => window.clearTimeout(timer);
  }, [dropNotice]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onDragDrop({ onHover: setDragging, onDrop: (paths) => void handleDropped(paths) })
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch((e) => {
        // Dinlenemezse bırakmalar sessizce kaybolurdu: nedeni hata günlüğüne yazılır.
        logFrontendError(`Sürükle-bırak dinlenemedi: ${describeError(e)}`).catch(() => {
          /* Günlüğe yazılamazsa yapılacak bir şey yok. */
        });
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [handleDropped]);

  useEffect(() => {
    let cancelled = false;
    getAppInfo()
      .then((value) => !cancelled && setInfo(value))
      .catch(() => {
        /* Çekirdeğe ulaşılamazsa yedek bilgi gösterilmeye devam eder. */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Klavye: 1–4 sahne seçer, Esc çekmeceyi kapatır (işaretleme sürerken Esc onu bitirir).
  const { recording } = marker;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      const picked = sceneForKey(event.key);
      if (picked) chooseScene(picked);
      else if (event.key === "Escape" && drawerOpen && !recording) closeDrawer();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseScene, closeDrawer, drawerOpen, recording]);

  const idle = useIdle(IDLE_MS);
  const cinema = idle && playing && !drawerOpen && !dragging;

  // Ekranda akıcı saatle ilerleyen konum kullanılır (sarmada anında güncellenir).
  const position = player.position;
  const shown = { ...status, positionSecs: position };
  const hasTrack = status.track !== null;
  const durationSecs = status.track?.durationSecs ?? null;
  const layout = useMemo(
    () =>
      songMap && durationSecs
        ? timelineLayout({
            durationSecs,
            sections: songMap.sections,
            programDrops: songMap.drops,
            markedDrops: [],
          })
        : null,
    [songMap, durationSecs],
  );
  // Ekran her karede yeniden çizilir: büyük kütüphanede aramayı her karede yapmamak için.
  const current = useMemo(
    () => libraryTrack(library.tracks, statusPath),
    [library.tracks, statusPath],
  );
  const queueLabel =
    queueActive && activeQueue.paths.length > 1
      ? `${activeQueue.index + 1} / ${activeQueue.paths.length}`
      : null;
  const upcomingTrack = useMemo(
    () => libraryTrack(library.tracks, upcoming),
    [library.tracks, upcoming],
  );
  const upNext: UpNext | null = upcoming
    ? {
        title: upcomingTrack?.title ?? fileStem(upcoming),
        artist: upcomingTrack?.artist ?? null,
        album: upcomingTrack?.album ?? null,
      }
    : null;

  let stage: ReactNode;
  switch (scene) {
    case "vu":
      stage = <VuScene playing={playing} />;
      break;
    case "highway":
      stage = <HighwayScene playing={playing} safe={visualSafe.safe} />;
      break;
    case "spectrum":
      stage = (
        <div className="spectrum-scene">
          {/* Şarkı açıkken gerçek spektrum; boştayken gösteri animasyonu. */}
          {hasTrack ? (
            <SpectrumView
              bands={SPECTRUM_BANDS}
              rows={SPECTRUM_ROWS}
              playing={playing}
              className="vfd vfd--primary"
            />
          ) : (
            <SpectrumDemo
              bands={SPECTRUM_BANDS}
              rows={SPECTRUM_ROWS}
              className="vfd vfd--primary"
            />
          )}
        </div>
      );
      break;
    default:
      stage = <SkyScene playing={playing} safe={visualSafe.safe} />;
  }

  const chromeButton = (id: Panel, label: string, icon: ReactNode, led?: boolean) => (
    <button
      type="button"
      className={`icon-button${drawerOpen && panel === id ? " is-active" : ""}`}
      aria-label={label}
      aria-expanded={drawerOpen && panel === id}
      aria-controls="drawer"
      title={label}
      onClick={() => togglePanel(id)}
    >
      {icon}
      {led !== undefined && <span className={`led${led ? " is-on" : ""}`} aria-hidden />}
    </button>
  );

  return (
    <main
      className={`app${cinema ? " is-cinema" : ""}${drawerOpen ? " has-drawer" : ""}${
        status.state === "error" || player.error ? " is-error" : ""
      }`}
    >
      <div className="stage" data-scene={scene}>
        {stage}
      </div>
      <div className="stage__shade" aria-hidden />

      <header className="topbar chrome">
        <div className="brand" title={`Lyraska ${info.version}`}>
          <LyraMark />
          <span className="brand__name">Lyraska</span>
        </div>
        <nav className="scenes" aria-label="Sahne">
          {SCENES.map(({ id, name }, index) => (
            <button
              key={id}
              type="button"
              className={`pill${scene === id ? " is-on" : ""}`}
              aria-pressed={scene === id}
              title={`${name} (${index + 1})`}
              onClick={() => chooseScene(id)}
            >
              {name}
            </button>
          ))}
        </nav>
        <div className="topbar__actions">
          {chromeButton("library", "Kütüphane", <LibraryIcon />)}
          {chromeButton("eq", "Ekolayzer", <SlidersIcon />, soundShaped)}
          {chromeButton("settings", "Ayarlar", <SettingsIcon />)}
        </div>
      </header>

      <div className="lower">
        <div className="hero chrome">
          <NowPlaying
            status={shown}
            album={current?.album ?? null}
            queueLabel={queueLabel}
            error={player.error}
            available={player.available}
          />
          <InfoStack
            status={status}
            positionSecs={position}
            songMap={songMap}
            soundShaped={soundShaped}
            safe={visualSafe.safe}
            next={upNext}
          />
        </div>

        <footer className="dock chrome">
          <SeekBar
            positionSecs={position}
            durationSecs={durationSecs}
            disabled={!player.available}
            onSeek={(seconds) => void player.seek(seconds)}
            layout={layout}
          />
          <div className="dock__row">
            <time className="dock__time">{hasTrack ? formatTime(position) : "--:--"}</time>
            <div className="transport">
              <button
                type="button"
                className="round-button"
                onClick={() => void player.stop()}
                disabled={!player.available || !hasTrack}
                title="Durdur ve başa dön"
                aria-label="Durdur"
              >
                <StopIcon />
              </button>
              <button
                type="button"
                className="round-button round-button--large"
                onClick={previous}
                disabled={!player.available || !status.track}
                title="Önceki şarkı (şarkının ortasındaysa başa sarar)"
                aria-label="Önceki"
              >
                <PreviousIcon />
              </button>
              <button
                type="button"
                className="play-button"
                onClick={() => void player.toggle()}
                disabled={!player.available || !hasTrack}
                title="Çal / Duraklat (Boşluk)"
                aria-label={playing ? "Duraklat" : "Çal"}
              >
                {playing ? <PauseIcon /> : <PlayIcon />}
              </button>
              <button
                type="button"
                className="round-button round-button--large"
                onClick={next}
                disabled={!player.available || !canNext}
                title="Sonraki şarkı"
                aria-label="Sonraki"
              >
                <NextIcon />
              </button>
              <button
                type="button"
                className="round-button"
                onClick={() => void player.openFile()}
                disabled={!player.available}
                title="Dosya aç (Ctrl+O)"
                aria-label="Dosya aç"
              >
                <EjectIcon />
              </button>
            </div>
            <time className="dock__time dock__time--end">
              {hasTrack ? formatTime(durationSecs ?? 0) : "--:--"}
            </time>
          </div>
        </footer>
      </div>

      <aside
        id="drawer"
        className={`drawer${drawerOpen ? " is-open" : ""}`}
        aria-label="Paneller"
        aria-hidden={!drawerOpen}
        inert={!drawerOpen}
      >
        <div className="drawer__head">
          <div className="drawer__tabs" role="tablist" aria-label="Paneller">
            {PANELS.map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                id={`drawer-tab-${id}`}
                aria-controls={`drawer-panel-${id}`}
                aria-selected={panel === id}
                className={`drawer__tab${panel === id ? " is-selected" : ""}`}
                onClick={() => selectPanel(id)}
              >
                {label}
                {id === "eq" && (
                  <span className={`led${soundShaped ? " is-on" : ""}`} aria-hidden />
                )}
              </button>
            ))}
          </div>
          <button
            type="button"
            className="icon-button icon-button--small"
            aria-label="Paneli kapat"
            title="Paneli kapat (Esc)"
            onClick={closeDrawer}
          >
            <CloseIcon />
          </button>
        </div>
        {/* Bütün paneller bağlı kalır: geçişte liste konumu ve seçim kaybolmaz. */}
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-library"
          aria-labelledby="drawer-tab-library"
          hidden={panel !== "library"}
        >
          <LibraryPanel
            library={library}
            available={player.available}
            extensions={info.supportedExtensions}
            currentPath={statusPath}
            playing={playing}
            onPlay={playFromLibrary}
          />
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-eq"
          aria-labelledby="drawer-tab-eq"
          hidden={panel !== "eq"}
        >
          <EqualizerPanel equalizer={equalizer} headphone={headphone} />
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-marker"
          aria-labelledby="drawer-tab-marker"
          hidden={panel !== "marker"}
        >
          <MarkerPanel
            marker={marker}
            trackTitle={status.track ? trackTitle(status.track) : null}
            playing={playing}
            durationSecs={durationSecs}
          />
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-sync"
          aria-labelledby="drawer-tab-sync"
          hidden={panel !== "sync"}
        >
          <SyncPanel sync={sync} playing={playing} available={player.available} />
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-settings"
          aria-labelledby="drawer-tab-settings"
          hidden={panel !== "settings"}
        >
          <SettingsPanel
            info={info}
            status={status}
            available={player.available}
            visualSafe={visualSafe}
          />
        </div>
      </aside>

      <DropOverlay dragging={dragging} notice={dropNotice} />
    </main>
  );
}
