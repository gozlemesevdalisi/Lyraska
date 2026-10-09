import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Dock } from "./Dock";
import { DropOverlay } from "./DropOverlay";
import { EqualizerPanel } from "./EqualizerPanel";
import { InfoStack, type UpNext } from "./InfoStack";
import { LibraryPanel } from "./LibraryPanel";
import { MarkerPanel } from "./MarkerPanel";
import { NowPlaying } from "./NowPlaying";
import { ErrorBoundary } from "./ErrorBoundary";
import { SceneStage } from "./SceneStage";
import { SettingsPanel } from "./SettingsPanel";
import { SyncPanel } from "./SyncPanel";
import { CloseIcon, LibraryIcon, LyraMark, SettingsIcon, SlidersIcon } from "./icons";
import {
  BROWSER_FALLBACK,
  getAppInfo,
  type AppInfo,
  type LibraryTrack,
  type SongMap,
} from "../lib/backend";
import { SCENES } from "../lib/scene";
import { fileStem, trackTitle } from "../lib/format";
import { themeAt } from "../lib/songMap";
import { timelineLayout, type TimelineLayout } from "../lib/timeline";
import { useDrawer, type Panel } from "../hooks/useDrawer";
import { useDropToLibrary } from "../hooks/useDropToLibrary";
import { useEqualizer } from "../hooks/useEqualizer";
import { useHeadphone } from "../hooks/useHeadphone";
import { useIdle } from "../hooks/useIdle";
import { usePlayback } from "../hooks/usePlayback";
import { useLivePosition, type PlayerControls } from "../hooks/usePlayer";
import { usePlaybackOptions } from "../hooks/usePlaybackOptions";
import { useScene } from "../hooks/useScene";
import { useSkyLook } from "../hooks/useSkyLook";
import { useCover } from "../hooks/useCover";
import { useSongMap } from "../hooks/useSongMap";
import { useSync } from "../hooks/useSync";
import { useVisualSafe } from "../hooks/useVisualSafe";
import { useMarker } from "../hooks/useMarker";
import { useLibrary } from "../hooks/useLibrary";

const PANELS: [Panel, string][] = [
  ["library", "Kütüphane"],
  ["eq", "Ekolayzer"],
  ["marker", "İşaretle"],
  ["sync", "Senkron"],
  ["settings", "Ayarlar"],
];

/** Müzik çalarken fare bu kadar kıpırdamazsa düğmeler çekilir, yalnızca sahne kalır. */
const IDLE_MS = 4000;

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
  const playbackOptions = usePlaybackOptions();
  const [scene, chooseScene] = useScene();
  const skyLook = useSkyLook();

  const playback = usePlayback(info.supportedExtensions);
  const { player, playList, upcoming } = playback;
  const { status } = player;
  // Ekolayzer ya da kulaklık düzeltmesi sesi değiştiriyorsa ekolayzer ışığı yanar.
  // Bit-perfect çalarken ikisi de sese uygulanmaz.
  const bitPerfect = status.output?.bitPerfect ?? false;
  const soundShaped = (equalizer.active || headphone.active) && !bitPerfect;
  const statusPath = status.track?.path ?? null;
  const playing = status.state === "playing";
  const songMap = useSongMap(statusPath);
  const sync = useSync(
    player.openPath,
    player.positionNow,
    status.state === "ended" ? statusPath : null,
  );
  const marker = useMarker(statusPath, playing, player.positionNow, songMap, sync.delayMs);

  // Panelden ayrılınca işaretleme biter (Boşluk yine çal/duraklat olur), senkron ölçümü de.
  const { setRecording } = marker;
  const { finishCalibration } = sync;
  const onLeave = useCallback(
    (panel: Panel) => {
      if (panel === "marker") setRecording(false);
      if (panel === "sync") finishCalibration();
    },
    [setRecording, finishCalibration],
  );
  const drawer = useDrawer({ onLeave, escapeBusy: marker.recording });

  // Çalmaya başlayınca sahne ortaya çıksın; kütüphane düğmesiyle geri açılır.
  const { close: closeDrawer } = drawer;
  const playAndReveal = useCallback(
    (paths: string[], start: string) => {
      playList(paths, start);
      closeDrawer();
    },
    [playList, closeDrawer],
  );
  const playFromLibrary = (track: LibraryTrack, list: LibraryTrack[]) =>
    playAndReveal(
      list.map((t) => t.path),
      track.path,
    );
  const drop = useDropToLibrary(
    library.addDropped,
    useCallback((tracks: string[]) => playAndReveal(tracks, tracks[0]!), [playAndReveal]),
  );

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

  const idle = useIdle(IDLE_MS);
  const cinema = idle && playing && !drawer.open && !drop.dragging;

  // Son bilinen konum (saniyede birkaç kez güncellenir). Akıcı konum yalnızca `LiveChrome`'da:
  // ekranın geri kalanı her karede yeniden çizilmez.
  const position = player.position;
  const hasTrack = status.track !== null;
  const durationSecs = status.track?.durationSecs ?? null;
  // Arayüzün vurgu renkleri çalan bölümün temasına (gökyüzüyle aynı) yavaşça geçer.
  const theme = themeAt(songMap?.sections ?? [], position);
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
  // Büyük kütüphanede aramayı her çizimde yapmamak için.
  const current = useMemo(
    () => libraryTrack(library.tracks, statusPath),
    [library.tracks, statusPath],
  );
  const upcomingTrack = useMemo(
    () => libraryTrack(library.tracks, upcoming),
    [library.tracks, upcoming],
  );
  const cover = useCover(statusPath);
  const upcomingCover = useCover(upcoming);
  const upNext: UpNext | null = upcoming
    ? {
        title: upcomingTrack?.title ?? fileStem(upcoming),
        artist: upcomingTrack?.artist ?? null,
        album: upcomingTrack?.album ?? null,
        cover: upcomingCover,
      }
    : null;

  const chromeButton = (id: Panel, label: string, icon: ReactNode, led?: boolean) => (
    <button
      type="button"
      className={`icon-button${drawer.open && drawer.panel === id ? " is-active" : ""}`}
      aria-label={label}
      aria-expanded={drawer.open && drawer.panel === id}
      aria-controls="drawer"
      title={label}
      onClick={() => drawer.toggle(id)}
    >
      {icon}
      {led !== undefined && <span className={`led${led ? " is-on" : ""}`} aria-hidden />}
    </button>
  );

  return (
    <main
      className={`app${cinema ? " is-cinema" : ""}${drawer.open ? " has-drawer" : ""}${
        status.state === "error" || player.error ? " is-error" : ""
      }`}
      data-theme={theme}
    >
      {/* Sahne çizilemezse yalnızca sahne yerine durgun gök kalır; başka sahne seçilince yeniden denenir. */}
      <ErrorBoundary
        key={`${scene}-${skyLook.look}`}
        name="Sahne"
        fallback={() => (
          <div className="stage" data-scene="error">
            <p className="stage__error" role="alert">
              Sahne çizilemedi; hata günlüğe yazıldı. Başka bir sahne seçebilirsiniz (1–4).
            </p>
          </div>
        )}
      >
        <SceneStage
          scene={scene}
          playing={playing}
          hasTrack={hasTrack}
          safe={visualSafe.safe}
          skyLook={skyLook.look}
        />
      </ErrorBoundary>
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

      <LiveChrome
        player={player}
        album={status.track?.album ?? current?.album ?? null}
        queueLabel={playback.queueLabel}
        cover={cover}
        songMap={songMap}
        soundShaped={soundShaped}
        safe={visualSafe.safe}
        next={upNext}
        layout={layout}
        canNext={playback.canNext}
        onPrevious={playback.previous}
        onNext={playback.next}
      />

      <aside
        id="drawer"
        className={`drawer${drawer.open ? " is-open" : ""}`}
        aria-label="Paneller"
        aria-hidden={!drawer.open}
        inert={!drawer.open}
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
                aria-selected={drawer.panel === id}
                className={`drawer__tab${drawer.panel === id ? " is-selected" : ""}`}
                onClick={() => drawer.select(id)}
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
            onClick={drawer.close}
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
          hidden={drawer.panel !== "library"}
        >
          <ErrorBoundary name="Kütüphane">
            <LibraryPanel
              library={library}
              available={player.available}
              extensions={info.supportedExtensions}
              currentPath={statusPath}
              playing={playing}
              onPlay={playFromLibrary}
            />
          </ErrorBoundary>
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-eq"
          aria-labelledby="drawer-tab-eq"
          hidden={drawer.panel !== "eq"}
        >
          <ErrorBoundary name="Ekolayzer">
            <EqualizerPanel equalizer={equalizer} headphone={headphone} bypassed={bitPerfect} />
          </ErrorBoundary>
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-marker"
          aria-labelledby="drawer-tab-marker"
          hidden={drawer.panel !== "marker"}
        >
          <ErrorBoundary name="İşaretleme">
            <MarkerPanel
              marker={marker}
              trackTitle={status.track ? trackTitle(status.track) : null}
              playing={playing}
              durationSecs={durationSecs}
            />
          </ErrorBoundary>
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-sync"
          aria-labelledby="drawer-tab-sync"
          hidden={drawer.panel !== "sync"}
        >
          <ErrorBoundary name="Senkron">
            <SyncPanel sync={sync} playing={playing} available={player.available} />
          </ErrorBoundary>
        </div>
        <div
          className="drawer__panel"
          role="tabpanel"
          id="drawer-panel-settings"
          aria-labelledby="drawer-tab-settings"
          hidden={drawer.panel !== "settings"}
        >
          <ErrorBoundary name="Ayarlar">
            <SettingsPanel
              info={info}
              status={status}
              available={player.available}
              visualSafe={visualSafe}
              playback={playbackOptions}
              skyLook={skyLook}
            />
          </ErrorBoundary>
        </div>
      </aside>

      <DropOverlay dragging={drop.dragging} notice={drop.notice} />
    </main>
  );
}

interface LiveChromeProps {
  player: PlayerControls;
  album: string | null;
  queueLabel: string | null;
  cover: string | null;
  songMap: SongMap | null;
  soundShaped: boolean;
  safe: boolean;
  next: UpNext | null;
  layout: TimelineLayout | null;
  canNext: boolean;
  onPrevious: () => void;
  onNext: () => void;
}

/**
 * Akıcı konumu gösteren cam katmanlar: büyük başlık, bilgi kartları (drop sayacı) ve alttaki
 * şarkı haritası şeridi. Çalarken her karede yalnızca bunlar yeniden çizilir.
 */
function LiveChrome({
  player,
  album,
  queueLabel,
  cover,
  songMap,
  soundShaped,
  safe,
  next,
  layout,
  canNext,
  onPrevious,
  onNext,
}: LiveChromeProps) {
  const position = useLivePosition(player);
  const { status } = player;
  return (
    <div className="lower">
      <div className="hero chrome">
        <NowPlaying
          status={{ ...status, positionSecs: position }}
          album={album}
          queueLabel={queueLabel}
          error={player.error}
          available={player.available}
          cover={cover}
        />
        <InfoStack
          status={status}
          positionSecs={position}
          songMap={songMap}
          soundShaped={soundShaped}
          safe={safe}
          next={next}
        />
      </div>

      <Dock
        player={player}
        position={position}
        layout={layout}
        canNext={canNext}
        onPrevious={onPrevious}
        onNext={onNext}
      />
    </div>
  );
}
