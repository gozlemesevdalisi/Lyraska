import { useEffect, useMemo, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { Marquee } from "./Marquee";
import { SeekBar } from "./SeekBar";
import { SpectrumDemo } from "./SpectrumDemo";
import { SpectrumView } from "./SpectrumView";
import { EjectIcon, PauseIcon, PlayIcon, StopIcon } from "./icons";
import { textToColumns } from "../lib/dotFont";
import { BROWSER_FALLBACK, getAppInfo, type AppInfo, type PlaybackStatus } from "../lib/backend";
import { formatTime, trackTechLine, trackTitle } from "../lib/format";
import { usePlayer } from "../hooks/usePlayer";

const TITLE = "LYRASKA";
const SPECTRUM_BANDS = 12;
/** Spektrumun sütun sayısı: her bant 2 sütun + aradaki 1 boşluk. */
const SPECTRUM_COLUMNS = SPECTRUM_BANDS * 3 - 1;
const WELCOME_TEXT =
  "HOŞ GELDİNİZ · BİR ŞARKI AÇMAK İÇİN DOSYA AÇ DÜĞMESİNE BASIN YA DA DOSYAYI PENCEREYE SÜRÜKLEYİN ·";

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
  return `${trackTitle(status.track)}${suffix} · ${trackTechLine(status.track)} ·`;
}

/**
 * Ana ekran: koyu bir ön panel, içinde eski araba teybi havasında parlayan
 * nokta matris ekran ve altında oynatıcı düğmeleri.
 */
export function PlayerScreen() {
  const [info, setInfo] = useState<AppInfo>(BROWSER_FALLBACK);
  const player = usePlayer(info.supportedExtensions);
  const { status } = player;

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

  const headlineText = headline(status);
  const headlineColumns = useMemo(() => textToColumns(headlineText), [headlineText]);
  const scrolling = marqueeText(status, player.error);
  const playing = status.state === "playing";
  const hasTrack = status.track !== null;
  const indicators = [
    { label: "ST", on: (status.track?.channels ?? 2) >= 2 && hasTrack },
    { label: playing ? "PLAY" : "PAUSE", on: hasTrack && status.state !== "ended" },
    { label: "64-BIT", on: true },
    { label: "EQ", on: false },
    { label: "BIT-PERFECT", on: false },
    { label: "SYNC", on: false },
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

          <div className="display__main">
            {/* Esneme oranları sütun sayılarına eşit: böylece başlık ve spektrum
                noktaları ekranda aynı boyutta görünür. */}
            <h1
              className="display__title"
              style={{ flexGrow: Math.max(headlineColumns.length, 41) }}
            >
              <DotMatrix
                columns={headlineColumns}
                label={hasTrack ? `Konum ${formatTime(status.positionSecs)}` : "Lyraska"}
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

          <Marquee
            key={scrolling}
            text={scrolling}
            width={150}
            className="vfd vfd--primary display__marquee"
          />

          <SeekBar
            positionSecs={status.positionSecs}
            durationSecs={status.track?.durationSecs ?? null}
            disabled={!player.available || player.busy}
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
              disabled={!player.available || player.busy}
              title="Dosya aç (Ctrl+O)"
              aria-label="Dosya aç"
            >
              <EjectIcon />
              <span>Dosya aç</span>
            </button>
            <button
              type="button"
              className="hw-button hw-button--primary"
              onClick={() => void player.toggle()}
              disabled={!player.available || !hasTrack || player.busy}
              title="Çal / Duraklat (Boşluk)"
              aria-label={playing ? "Duraklat" : "Çal"}
            >
              {playing ? <PauseIcon /> : <PlayIcon />}
              <span>{playing ? "Duraklat" : "Çal"}</span>
            </button>
            <button
              type="button"
              className="hw-button"
              onClick={() => void player.stop()}
              disabled={!player.available || !hasTrack || player.busy}
              title="Durdur ve başa dön"
              aria-label="Durdur"
            >
              <StopIcon />
              <span>Durdur</span>
            </button>
          </div>
          <span className="knob" aria-hidden />
        </div>
      </section>

      <footer className="status" aria-live="polite">
        <span>
          {info.phase} · Sürüm {info.version}
        </span>
        {hasTrack && status.track ? (
          <>
            <span>
              {formatTime(status.positionSecs)} / {formatTime(status.track.durationSecs ?? 0)}
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
      </footer>
    </main>
  );
}
