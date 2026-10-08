import { useCallback, useEffect, useMemo, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { DropOverlay } from "./DropOverlay";
import { Marquee } from "./Marquee";
import { EqualizerPanel } from "./EqualizerPanel";
import { LibraryPanel } from "./LibraryPanel";
import { MarkerPanel } from "./MarkerPanel";
import { SeekBar } from "./SeekBar";
import { SpectrumDemo } from "./SpectrumDemo";
import { SpectrumView } from "./SpectrumView";
import { HighwayScene } from "./HighwayScene";
import { SkyScene } from "./SkyScene";
import { SyncPanel } from "./SyncPanel";
import { VuScene } from "./VuScene";
import { EjectIcon, NextIcon, PauseIcon, PlayIcon, PreviousIcon, StopIcon } from "./icons";
import { textToColumns } from "../lib/dotFont";
import {
  BROWSER_FALLBACK,
  getAppInfo,
  logFrontendError,
  onDragDrop,
  errorMessage,
  openLog,
  setNextTrack,
  type AppInfo,
  type LibraryTrack,
  type PlaybackStatus,
} from "../lib/backend";
import { DROP_NOTICE_MS, DROP_PROBLEM_MS, dropSummary, type DropNotice } from "../lib/drop";
import { describeError } from "../lib/errorReporting";
import { loadScene, nextScene, saveScene, sceneName, type Scene } from "../lib/scene";
import { formatBpm, formatTime, signalPathText, trackTechLine, trackTitle } from "../lib/format";
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
import { useSync } from "../hooks/useSync";
import { useVisualSafe } from "../hooks/useVisualSafe";
import { useMarker } from "../hooks/useMarker";
import { useLibrary } from "../hooks/useLibrary";
import { usePlayer } from "../hooks/usePlayer";

const TITLE = "LYRASKA";
type Deck = "library" | "eq" | "marker" | "sync";
const SPECTRUM_BANDS = 12;
/** Spektrumun sütun sayısı: her bant 2 sütun + aradaki 1 boşluk. */
const SPECTRUM_COLUMNS = SPECTRUM_BANDS * 3 - 1;
const WELCOME_TEXT = "HOŞ GELDİNİZ · MÜZİK KLASÖRÜNÜZÜ YA DA ŞARKILARINIZI PENCEREYE SÜRÜKLEYİN ·";

/** Büyük nokta matris alanında gösterilecek metin: boşta program adı, çalarken süre. */
export function headline(status: PlaybackStatus): string {
  if (!status.track) return TITLE;
  const time = formatTime(status.positionSecs);
  switch (status.state) {
    case "playing":
      return `▶ ${time}`;
    case "paused":
      // Durdur düğmesi şarkıyı başa sarıp duraklatır; bunu ■ ile göster.
      return status.positionSecs < 0.05 ? `■ ${time}` : `‖ ${time}`;
    case "ended":
      return `■ ${formatTime(status.track.durationSecs ?? status.positionSecs)}`;
    default:
      return `■ ${time}`;
  }
}

/** Kayan yazıda gösterilecek metin. */
export function marqueeText(status: PlaybackStatus, error: string | null): string {
  const problem = error ?? status.error;
  if (problem) return `HATA · ${problem} ·`;
  if (!status.track) return WELCOME_TEXT;
  const suffix = status.state === "ended" ? " · BİTTİ" : "";
  const tempo = status.bpm ? ` · ${formatBpm(status.bpm)}` : "";
  return `${trackTitle(status.track)}${suffix} · ${trackTechLine(status.track)}${tempo} ·`;
}

/**
 * Ana ekran: koyu bir ön panel, içinde eski araba teybi havasında parlayan
 * nokta matris ekran ve altında oynatıcı düğmeleri.
 */
export function PlayerScreen() {
  const [info, setInfo] = useState<AppInfo>(BROWSER_FALLBACK);
  const library = useLibrary();
  const equalizer = useEqualizer();
  const headphone = useHeadphone();
  const visualSafe = useVisualSafe();
  // "EQ" ışığı: ekolayzer ya da kulaklık düzeltmesi sesi değiştiriyorsa yanar.
  const soundShaped = equalizer.active || headphone.active;
  const [deck, setDeck] = useState<Deck>("library");
  const [logError, setLogError] = useState<string | null>(null);
  const [scene, setScene] = useState<Scene>(loadScene);
  const changeScene = () => {
    const next = nextScene(scene);
    setScene(next);
    saveScene(next);
  };
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

  const playFromLibrary = (track: LibraryTrack, list: LibraryTrack[]) =>
    playQueue(
      queueFrom(
        list.map((t) => t.path),
        track.path,
      ),
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

  // Pencereye bırakılanlar: şarkılar ve klasörler kütüphaneye eklenir, şarkılar bırakılış
  // sırasıyla çalınır. Dosya mı klasör mü olduğuna çekirdek diskte bakar. Sonuç (sorunlar
  // dahil) hangi sekme açık olursa olsun kısa bir bildirimle gösterilir.
  const { addDropped } = library;
  const [dragging, setDragging] = useState(false);
  const [dropNotice, setDropNotice] = useState<DropNotice | null>(null);
  const handleDropped = useCallback(
    async (paths: string[]) => {
      let notice: DropNotice;
      try {
        const outcome = await addDropped(paths);
        const first = outcome.tracks[0];
        if (first) playQueue(queueFrom(outcome.tracks, first));
        notice = dropSummary(outcome);
      } catch (e) {
        notice = { text: errorMessage(e), problem: true };
      }
      setDropNotice(notice);
    },
    [addDropped, playQueue],
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

  // Ekranda akıcı saatle ilerleyen konum kullanılır (sarmada anında güncellenir).
  const shown = { ...status, positionSecs: player.position };
  const headlineText = headline({ ...status, positionSecs: player.position });
  const headlineColumns = useMemo(() => textToColumns(headlineText), [headlineText]);
  const scrolling = marqueeText(status, player.error);
  const playing = status.state === "playing";
  const marker = useMarker(statusPath, playing, player.positionNow);
  const sync = useSync(
    player.openPath,
    player.positionNow,
    status.state === "ended" ? statusPath : null,
  );
  const hasTrack = status.track !== null;
  const signalPath = signalPathText(status);
  const headlineLabel = hasTrack ? `Konum ${formatTime(shown.positionSecs)}` : "Lyraska";
  // VU ve gece göğü sahnelerinde süre ortada/önde gösterilir.
  const headlineMatrix = (
    <DotMatrix columns={headlineColumns} label={headlineLabel} className="vfd vfd--primary" />
  );
  const indicators = [
    { label: "ST", on: (status.track?.channels ?? 2) >= 2 && hasTrack },
    { label: playing ? "PLAY" : "PAUSE", on: hasTrack && status.state !== "ended" },
    { label: "64-BIT", on: true },
    { label: "EQ", on: soundShaped },
    { label: "BIT-PERFECT", on: false },
    { label: "SYNC", on: false },
    { label: "SAFE", on: visualSafe.safe },
  ];

  return (
    <main className="welcome">
      <section className="faceplate" aria-label="Lyraska oynatıcı">
        <div className="faceplate__screw faceplate__screw--left" aria-hidden />
        <div className="faceplate__screw faceplate__screw--right" aria-hidden />

        <div className={`display${status.state === "error" || player.error ? " is-error" : ""}`}>
          <div className="display__glass" aria-hidden />

          <ul className="display__indicators" aria-hidden>
            {indicators.map(({ label, on }) => (
              <li key={label} className={on ? "is-on" : undefined}>
                {label}
              </li>
            ))}
          </ul>

          {scene === "vu" ? (
            <div className="display__main display__main--vu">
              <VuScene playing={playing} center={headlineMatrix} />
            </div>
          ) : scene === "sky" ? (
            <div className="display__main display__main--sky">
              <SkyScene playing={playing} safe={visualSafe.safe} center={headlineMatrix} />
            </div>
          ) : scene === "highway" ? (
            <div className="display__main display__main--sky">
              <HighwayScene playing={playing} safe={visualSafe.safe} center={headlineMatrix} />
            </div>
          ) : (
            <div className="display__main">
              {/* Esneme oranları sütun sayılarına eşit: böylece başlık ve spektrum
                  noktaları ekranda aynı boyutta görünür. */}
              <h1
                className="display__title"
                style={{ flexGrow: Math.max(headlineColumns.length, 41) }}
              >
                <DotMatrix
                  columns={headlineColumns}
                  label={headlineLabel}
                  className="vfd vfd--primary"
                />
              </h1>
              <div className="display__spectrum" style={{ flexGrow: SPECTRUM_COLUMNS }}>
                {/* Şarkı açıkken gerçek spektrum; boştayken gösteri animasyonu. */}
                {hasTrack ? (
                  <SpectrumView
                    bands={SPECTRUM_BANDS}
                    rows={8}
                    playing={playing}
                    className="vfd vfd--accent"
                  />
                ) : (
                  <SpectrumDemo bands={SPECTRUM_BANDS} rows={8} className="vfd vfd--accent" />
                )}
              </div>
            </div>
          )}

          <Marquee
            key={scrolling}
            text={scrolling}
            width={150}
            className="vfd vfd--primary display__marquee"
          />

          <SeekBar
            positionSecs={shown.positionSecs}
            durationSecs={status.track?.durationSecs ?? null}
            disabled={!player.available}
            onSeek={(seconds) => void player.seek(seconds)}
          />
        </div>

        <div className="faceplate__controls">
          <span className="knob" aria-hidden />
          <div className="transport">
            <button
              type="button"
              className="hw-button"
              onClick={() => void player.openFile()}
              disabled={!player.available}
              title="Dosya aç (Ctrl+O)"
              aria-label="Dosya aç"
            >
              <EjectIcon />
              <span>Dosya aç</span>
            </button>
            <button
              type="button"
              className="hw-button hw-button--icon"
              onClick={previous}
              disabled={!player.available || !status.track}
              title="Önceki şarkı (şarkının ortasındaysa başa sarar)"
              aria-label="Önceki"
            >
              <PreviousIcon />
            </button>
            <button
              type="button"
              className="hw-button hw-button--primary"
              onClick={() => void player.toggle()}
              disabled={!player.available || !hasTrack}
              title="Çal / Duraklat (Boşluk)"
              aria-label={playing ? "Duraklat" : "Çal"}
            >
              {playing ? <PauseIcon /> : <PlayIcon />}
              <span>{playing ? "Duraklat" : "Çal"}</span>
            </button>
            <button
              type="button"
              className="hw-button hw-button--icon"
              onClick={next}
              disabled={!player.available || !canNext}
              title="Sonraki şarkı"
              aria-label="Sonraki"
            >
              <NextIcon />
            </button>
            <button
              type="button"
              className="hw-button"
              onClick={() => void player.stop()}
              disabled={!player.available || !hasTrack}
              title="Durdur ve başa dön"
              aria-label="Durdur"
            >
              <StopIcon />
              <span>Durdur</span>
            </button>
          </div>
          <button
            type="button"
            className="knob knob--button"
            data-scene={scene}
            onClick={changeScene}
            aria-label={`Sahne: ${sceneName(scene)}. Değiştirmek için basın.`}
            title={`Sahne: ${sceneName(scene)} (değiştirmek için basın)`}
          />
        </div>
      </section>

      <div className="deck">
        <div className="deck__bar">
          <div className="deck__tabs" role="tablist" aria-label="Alt panel">
            {(
              [
                ["library", "Kütüphane"],
                ["eq", "Ekolayzer"],
                ["marker", "İşaretle"],
                ["sync", "Senkron"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                id={`deck-tab-${id}`}
                aria-controls={`deck-panel-${id}`}
                aria-selected={deck === id}
                className={`deck__tab${deck === id ? " is-selected" : ""}`}
                onClick={() => {
                  setDeck(id);
                  // Sekmeden çıkınca işaretleme biter: Boşluk yine çal/duraklat olur.
                  if (id !== "marker") marker.setRecording(false);
                  // Senkron ölçümü de sekmeden çıkınca biter.
                  if (id !== "sync") sync.finishCalibration();
                }}
              >
                {label}
                {id === "eq" && (
                  <span className={`deck__led${soundShaped ? " is-on" : ""}`} aria-hidden />
                )}
              </button>
            ))}
          </div>
          {signalPath && (
            <p
              className="signal-path"
              title="Sinyal yolu: sesin şarkı dosyasından hoparlöre nasıl gittiği. Şarkı, ses aygıtının kendi hızına Lyraska'nın stüdyo kalitesindeki dönüştürücüsüyle çevrilir; Windows sese dokunmaz. Taşma koruması, yüksek kayıtlardaki tepelerin cızırtı yapmasını önler."
            >
              <span className="signal-path__label">Sinyal yolu</span> {signalPath}
            </p>
          )}
        </div>
        {/* İki panel de bağlı kalır: geçişte liste konumu ve seçim kaybolmaz. */}
        <div
          className="deck__panel"
          role="tabpanel"
          id="deck-panel-library"
          aria-labelledby="deck-tab-library"
          hidden={deck !== "library"}
        >
          <LibraryPanel
            library={library}
            available={player.available}
            extensions={info.supportedExtensions}
            currentPath={status.track?.path ?? null}
            playing={playing}
            onPlay={playFromLibrary}
          />
        </div>
        <div
          className="deck__panel"
          role="tabpanel"
          id="deck-panel-eq"
          aria-labelledby="deck-tab-eq"
          hidden={deck !== "eq"}
        >
          <EqualizerPanel equalizer={equalizer} headphone={headphone} />
        </div>
        <div
          className="deck__panel"
          role="tabpanel"
          id="deck-panel-marker"
          aria-labelledby="deck-tab-marker"
          hidden={deck !== "marker"}
        >
          <MarkerPanel
            marker={marker}
            trackTitle={status.track ? trackTitle(status.track) : null}
            playing={playing}
            durationSecs={status.track?.durationSecs ?? null}
          />
        </div>
        <div
          className="deck__panel"
          role="tabpanel"
          id="deck-panel-sync"
          aria-labelledby="deck-tab-sync"
          hidden={deck !== "sync"}
        >
          <SyncPanel sync={sync} playing={playing} available={player.available} />
        </div>
      </div>

      <footer className="status" aria-live="polite">
        <span>
          {info.phase} · Sürüm {info.version}
        </span>
        {hasTrack && status.track ? (
          <>
            <span>
              {formatTime(shown.positionSecs)} / {formatTime(status.track.durationSecs ?? 0)}
            </span>
            <span>{trackTechLine(status.track)}</span>
            <span className={status.underruns > 0 ? "status__warn" : undefined}>
              Kesinti: {status.underruns}
            </span>
          </>
        ) : (
          <span>Ses motoru: {info.audioEngine}</span>
        )}
        {!player.available && <span>Ses çalmak için programı Windows'ta açın.</span>}
        {player.available && (
          <button
            type="button"
            className="status__link"
            title="Hata ve çökme günlüğünü dosya gezgininde gösterir. Hata kaydı açarken bu dosyayı ekleyin."
            onClick={() => {
              openLog().catch((e) => setLogError(errorMessage(e)));
            }}
          >
            Hata günlüğü
          </button>
        )}
        <button
          type="button"
          className="status__link"
          aria-pressed={visualSafe.safe}
          title="Açıkken görseller daha sakin olur: saniyede en fazla bir vuruş, parlaklık daha yavaş değişir."
          onClick={() => void visualSafe.setSafe(!visualSafe.safe)}
        >
          Epilepsi güvenli modu: {visualSafe.safe ? "Açık" : "Kapalı"}
        </button>
        {visualSafe.error && <span className="status__warn">{visualSafe.error}</span>}
        {logError && <span className="status__warn">{logError}</span>}
      </footer>
      <DropOverlay dragging={dragging} notice={dropNotice} />
    </main>
  );
}
