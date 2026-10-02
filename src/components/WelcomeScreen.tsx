import { useEffect, useMemo, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { Marquee } from "./Marquee";
import { SpectrumDemo } from "./SpectrumDemo";
import { textToColumns } from "../lib/dotFont";
import { BROWSER_FALLBACK, getAppInfo, type AppInfo } from "../lib/backend";

const TITLE = "LYRASKA";
const SPECTRUM_BANDS = 12;
/** Spektrumun sütun sayısı: her bant 2 sütun + aradaki 1 boşluk. */
const SPECTRUM_COLUMNS = SPECTRUM_BANDS * 3 - 1;
const MARQUEE_TEXT =
  "HOŞ GELDİNİZ · GÖRSELLER ŞARKIYI ÖNCEDEN BİLİR · TAMAMEN ÇEVRİMDIŞI · FAZ 0: TEMEL KURULDU ·";

/** Ekranın üst kenarındaki küçük gösterge yazıları (yanık / sönük). */
const INDICATORS: { label: string; on: boolean }[] = [
  { label: "ST", on: true },
  { label: "LOUD", on: false },
  { label: "EQ", on: true },
  { label: "64-BIT", on: true },
  { label: "BIT-PERFECT", on: false },
  { label: "SYNC", on: false },
];

/**
 * Program açıldığında görünen karşılama ekranı: koyu bir ön panel ve
 * içinde parlayan, eski araba teybi havasında bir nokta matris ekran.
 */
export function WelcomeScreen() {
  const [info, setInfo] = useState<AppInfo>(BROWSER_FALLBACK);
  const titleColumns = useMemo(() => textToColumns(TITLE), []);

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

  return (
    <main className="welcome">
      <section className="faceplate" aria-label="Karşılama ekranı">
        <div className="faceplate__screw faceplate__screw--left" aria-hidden />
        <div className="faceplate__screw faceplate__screw--right" aria-hidden />

        <div className="display">
          <div className="display__glass" aria-hidden />

          <ul className="display__indicators" aria-hidden>
            {INDICATORS.map(({ label, on }) => (
              <li key={label} className={on ? "is-on" : undefined}>
                {label}
              </li>
            ))}
          </ul>

          <div className="display__main">
            {/* Esneme oranları sütun sayılarına eşit: böylece başlık ve spektrum
                noktaları ekranda aynı boyutta görünür. */}
            <h1 className="display__title" style={{ flexGrow: titleColumns.length }}>
              <DotMatrix columns={titleColumns} label="Lyraska" className="vfd vfd--primary" />
            </h1>
            <div className="display__spectrum" style={{ flexGrow: SPECTRUM_COLUMNS }}>
              <SpectrumDemo bands={SPECTRUM_BANDS} rows={8} className="vfd vfd--accent" />
            </div>
          </div>

          <Marquee text={MARQUEE_TEXT} width={150} className="vfd vfd--primary display__marquee" />
        </div>

        <div className="faceplate__controls" aria-hidden>
          <span className="knob" />
          <span className="faceplate__brand">özgün tasarım · çevrimdışı</span>
          <span className="knob" />
        </div>
      </section>

      <footer className="status" aria-live="polite">
        <span>
          {info.phase} · Sürüm {info.version}
        </span>
        <span>Ses motoru: {info.audioEngine}</span>
        <span>Analiz: {info.analysis}</span>
        <span>Görsel köprü: {info.visualBridge}</span>
      </footer>
    </main>
  );
}
