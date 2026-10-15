import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { AbxStatus } from "../lib/backend";
import { useAbx } from "../hooks/useAbx";
import { AbxPanel } from "./AbxPanel";

// Rust çekirdeğinin kör test komutlarını taklit eden sahte arka uç.
const core = vi.hoisted(() => ({
  status: null as AbxStatus | null,
  picks: [] as (string | null)[],
  start: vi.fn(),
  select: vi.fn(),
  answer: vi.fn(),
  pause: vi.fn(),
  stop: vi.fn(),
  startError: null as string | null,
  /** Durum sorguları: kaç kez soruldu; `hold` açıkken yanıtlar bekletilir. */
  statusCalls: 0,
  hold: false,
  held: [] as (() => void)[],
}));

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  const now = () => core.status ?? actual.ABX_INACTIVE;
  return {
    ...actual,
    pickAbxFile: async () => core.picks.shift() ?? null,
    abxStatus: () => {
      core.statusCalls++;
      const snapshot = now();
      if (!core.hold) return Promise.resolve(snapshot);
      return new Promise<AbxStatus>((resolve) => core.held.push(() => resolve(snapshot)));
    },
    abxStart: async (request: import("../lib/backend").AbxRequest) => {
      core.start(request);
      if (core.startError) throw core.startError;
      core.status = running();
      return now();
    },
    abxSelect: async (choice: import("../lib/backend").AbxChoice) => {
      core.select(choice);
      core.status = { ...now(), playing: choice };
      return now();
    },
    abxPause: async () => {
      core.pause();
      core.status = { ...now(), playing: null };
      return now();
    },
    abxAnswer: async (guess: import("../lib/backend").AbxSource) => {
      core.answer(guess);
      core.status = { ...now(), answered: now().answered + 1 };
      return now();
    },
    abxStop: async () => {
      core.stop();
      core.status = null;
      return actual.ABX_INACTIVE;
    },
  };
});

function running(): AbxStatus {
  return {
    active: true,
    aName: "Şarkı.flac",
    bName: "Şarkı.mp3",
    startSecs: 30,
    clipSecs: 20,
    positionSecs: 5,
    playing: null,
    answered: 0,
    total: 16,
    result: null,
    levelDiffDb: 0.3,
    offsetMs: 25.06,
    similarity: 0.998,
    sampleRate: 48000,
    error: null,
  };
}

function Harness({ visible = true }: { visible?: boolean }) {
  const abx = useAbx(["flac", "mp3"], visible);
  return <AbxPanel abx={abx} available />;
}

beforeEach(() => {
  core.status = null;
  core.picks = [];
  core.startError = null;
  core.statusCalls = 0;
  core.hold = false;
  core.held = [];
  for (const fn of [core.start, core.select, core.answer, core.pause, core.stop]) fn.mockClear();
});

describe("Kör test paneli", () => {
  it("iki dosya seçilmeden başlamaz, seçilince ayarlarla başlar", async () => {
    render(<Harness />);
    const start = screen.getByRole("button", { name: "Testi başlat" });
    expect((start as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/16 denemede en az 12 doğru/)).toBeTruthy();

    core.picks = ["C:\\Müzik\\Şarkı.flac", "C:\\Müzik\\Şarkı.mp3"];
    fireEvent.click(screen.getByRole("button", { name: "A dosyasını seç" }));
    await screen.findByText("Şarkı.flac");
    fireEvent.click(screen.getByRole("button", { name: "B dosyasını seç" }));
    await screen.findByText("Şarkı.mp3");
    fireEvent.change(screen.getByLabelText("Süre"), { target: { value: "10" } });
    fireEvent.change(screen.getByLabelText("Deneme"), { target: { value: "8" } });
    expect(screen.getByText(/8 denemede en az 7 doğru/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Testi başlat" }));

    await screen.findByText(/Deneme 1 \/ 16/);
    expect(core.start).toHaveBeenCalledWith({
      a: "C:\\Müzik\\Şarkı.flac",
      b: "C:\\Müzik\\Şarkı.mp3",
      startSecs: 30,
      clipSecs: 10,
      trials: 8,
    });
    // Hizalama ve eşitleme kullanıcıya söylenir.
    expect(
      screen.getByText(/seviye farkı 0,3 dB eşitlendi · kayma 25,1 ms düzeltildi/),
    ).toBeTruthy();
  });

  it("A, B, X çalar; yanıtlar sayılır; sonuç sonda gösterilir", async () => {
    core.status = running();
    render(<Harness />);
    await screen.findByText(/Deneme 1 \/ 16/);

    fireEvent.click(screen.getByRole("button", { name: "X" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "X" }).getAttribute("aria-pressed")).toBe("true"),
    );
    expect(core.select).toHaveBeenCalledWith("x");

    fireEvent.click(screen.getByRole("button", { name: "X = A" }));
    await screen.findByText(/Deneme 2 \/ 16/);
    expect(core.answer).toHaveBeenCalledWith("a");

    fireEvent.click(screen.getByRole("button", { name: "Duraklat" }));
    await waitFor(() => expect(core.pause).toHaveBeenCalled());

    // Bütün denemeler bitti: sonuç.
    act(() => {
      core.status = {
        ...running(),
        answered: 16,
        result: { correct: 12, total: 16, pValue: 2517 / 65536 },
      };
    });
    await screen.findByText("Farkı duyuyorsunuz", undefined, { timeout: 2000 });
    expect(screen.getByText(/16 denemede 12 doğru/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Kapat" }));
    await screen.findByRole("button", { name: "Testi başlat" });
    expect(core.stop).toHaveBeenCalled();
  });

  it("kurulamazsa nedenini gösterir", async () => {
    core.startError = "Kör test başlatılamadı: İki dosya aynı kayda benzemiyor (benzerlik %3).";
    core.picks = ["a.flac", "b.mp3"];
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "A dosyasını seç" }));
    await screen.findByText("a.flac");
    fireEvent.click(screen.getByRole("button", { name: "B dosyasını seç" }));
    await screen.findByText("b.mp3");
    fireEvent.click(screen.getByRole("button", { name: "Testi başlat" }));
    await screen.findByText(/aynı kayda benzemiyor/);
    expect(screen.getByRole("button", { name: "Testi başlat" })).toBeTruthy();
  });

  it("durum sorguları birikmez, geç gelen eski yanıt denemeyi geri almaz", async () => {
    core.status = running();
    render(<Harness />);
    await screen.findByText(/Deneme 1 \/ 16/);

    // Çekirdek meşgul: yanıt gelmeden yeni sorgu gönderilmez.
    core.hold = true;
    await waitFor(() => expect(core.held.length).toBe(1));
    const calls = core.statusCalls;
    await act(() => new Promise((resolve) => setTimeout(resolve, 700)));
    expect(core.statusCalls).toBe(calls);

    // Bu arada yanıt verildi; bekleyen eski sorgunun yanıtı sonra gelir.
    core.hold = false;
    fireEvent.click(screen.getByRole("button", { name: "X = A" }));
    await screen.findByText(/Deneme 2 \/ 16/);
    await act(async () => {
      core.held.shift()?.();
      await Promise.resolve();
    });
    expect(screen.getByText(/Deneme 2 \/ 16/)).toBeTruthy();
  });

  it("panelden ayrılınca çalan test duraklatılır", async () => {
    core.status = { ...running(), playing: "b" };
    const { rerender } = render(<Harness />);
    await screen.findByText(/Deneme 1 \/ 16/);
    rerender(<Harness visible={false} />);
    await waitFor(() => expect(core.pause).toHaveBeenCalled());
  });
});
