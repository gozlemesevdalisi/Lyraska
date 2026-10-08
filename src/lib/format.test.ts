import {
  fileStem,
  formatBpm,
  formatLabel,
  formatTime,
  progress,
  signalPathText,
  titleScale,
  trackName,
  trackTechLine,
  trackTitle,
} from "./format";
import type { PlaybackStatus, TrackInfo } from "./backend";

const track: TrackInfo = {
  path: "C:\\Müzik\\sarki.flac",
  fileName: "sarki",
  title: null,
  artist: null,
  codec: "flac",
  sampleRate: 44100,
  channels: 2,
  durationSecs: 225,
};

describe("formatTime", () => {
  it("dakika ve saniyeyi iki haneli yazar", () => {
    expect(formatTime(0)).toBe("00:00");
    expect(formatTime(5.9)).toBe("00:05");
    expect(formatTime(225)).toBe("03:45");
  });

  it("bir saatten uzun süreleri saatle yazar", () => {
    expect(formatTime(3723)).toBe("1:02:03");
  });

  it("geçersiz değerleri sıfır sayar", () => {
    expect(formatTime(-3)).toBe("00:00");
    expect(formatTime(Number.NaN)).toBe("00:00");
  });
});

describe("trackTitle", () => {
  it("sanatçı ve başlığı birleştirir, yoksa dosya adına düşer", () => {
    expect(trackTitle({ ...track, title: "Gece", artist: "Lyra" })).toBe("Lyra - Gece");
    expect(trackTitle({ ...track, title: "Gece" })).toBe("Gece");
    expect(trackTitle(track)).toBe("sarki");
  });
});

describe("trackTechLine", () => {
  it("kodek, örnekleme hızı ve kanalı Türkçe biçimde yazar", () => {
    expect(trackTechLine(track)).toBe("FLAC · 44,1 kHz · Stereo");
    expect(trackTechLine({ ...track, codec: "mp3", sampleRate: 48000, channels: 1 })).toBe(
      "MP3 · 48 kHz · Mono",
    );
    expect(trackTechLine({ ...track, channels: 6 })).toContain("6 kanal");
  });
});

describe("signalPathText", () => {
  const status: PlaybackStatus = {
    state: "playing",
    track: { ...track, codec: "mp3" },
    positionSecs: 1,
    underruns: 0,
    error: null,
    bpm: null,
    output: {
      deviceName: "Hoparlörler (Realtek)",
      sampleRate: 48000,
      channels: 2,
      resampled: true,
    },
  };

  it("sesin aygıta giden yolunu anlatır", () => {
    expect(signalPathText(status)).toBe(
      "MP3 44,1 kHz → 48 kHz (yüksek kalite) → Hoparlörler (Realtek) · taşma koruması",
    );
  });

  it("dönüştürme yoksa, aygıt adı bilinmiyorsa ve takılma varsa söyler", () => {
    const text = signalPathText({
      ...status,
      underruns: 3,
      output: { deviceName: "", sampleRate: 44100, channels: 2, resampled: false },
    });
    expect(text).toContain("dönüştürmesiz → varsayılan ses aygıtı");
    expect(text).toContain("takılma: 3");
    expect(signalPathText({ ...status, output: null })).toBeNull();
    expect(signalPathText({ ...status, track: null })).toBeNull();
  });
});

describe("formatBpm", () => {
  it("tempoyu Türkçe ondalıkla yazar", () => {
    expect(formatBpm(128)).toBe("128 BPM");
    expect(formatBpm(105.5)).toBe("105,5 BPM");
    expect(formatBpm(89.96)).toBe("90 BPM");
  });
});

describe("progress", () => {
  it("oranı 0..1 aralığında tutar", () => {
    expect(progress(50, 200)).toBe(0.25);
    expect(progress(300, 200)).toBe(1);
    expect(progress(10, null)).toBe(0);
    expect(progress(10, 0)).toBe(0);
  });
});

describe("büyük başlık", () => {
  it("etiketteki başlığı, yoksa dosya adını gösterir", () => {
    expect(trackName(track)).toBe("sarki");
    expect(trackName({ ...track, title: "Gülümse", artist: "Sezen Aksu" })).toBe("Gülümse");
    expect(trackName({ ...track, title: "  " })).toBe("sarki");
  });

  it("uzun adlarda yazı küçülür", () => {
    expect(titleScale("Gülümse")).toBe("xl");
    expect(titleScale("Sözüm Meclisten Dışarı")).toBe("l");
    expect(titleScale("Bir Şarkının Çok Uzun Olabilen Adı (Canlı Kayıt)")).toBe("m");
  });
});

describe("ses biçimi etiketi", () => {
  it("dönüştürme varsa iki hızı da yazar", () => {
    const output = { deviceName: "", sampleRate: 48000, channels: 2, resampled: true };
    expect(formatLabel(track, output)).toBe("FLAC 44,1 → 48 kHz");
  });

  it("dönüştürme yoksa ya da çıkış bilinmiyorsa yalnızca şarkının hızını yazar", () => {
    const output = { deviceName: "", sampleRate: 44100, channels: 2, resampled: false };
    expect(formatLabel(track, output)).toBe("FLAC 44,1 kHz");
    expect(formatLabel({ ...track, codec: "mp3", sampleRate: 48000 }, null)).toBe("MP3 48 kHz");
  });
});

describe("dosya adı", () => {
  it("yoldan uzantısız adı çıkarır", () => {
    expect(fileStem("D:\\Müzik\\Firuze.flac")).toBe("Firuze");
    expect(fileStem("/home/ali/Sezen Aksu - Gülümse.mp3")).toBe("Sezen Aksu - Gülümse");
    expect(fileStem("C:\\Müzik\\.gizli")).toBe(".gizli");
    expect(fileStem("adsız")).toBe("adsız");
  });
});
