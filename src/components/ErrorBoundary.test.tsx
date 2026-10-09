import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const logged = vi.hoisted(() => ({ messages: [] as string[] }));
vi.mock("../lib/backend", () => ({
  logFrontendError: async (message: string) => {
    logged.messages.push(message);
  },
}));

import { ErrorBoundary } from "./ErrorBoundary";

const state = { broken: true };

function Fragile() {
  if (state.broken) throw new TypeError("Cannot read properties of undefined (reading 'toFixed')");
  return <p>Panel çalışıyor</p>;
}

describe("hata sınırı", () => {
  beforeEach(() => {
    // React yakalanan hatayı konsola da yazar; test çıktısı kirlenmesin.
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    logged.messages = [];
    state.broken = true;
  });
  afterEach(() => vi.restoreAllMocks());

  it("bölüm hata verirse yalnızca o bölüm 'açılamadı' der, gerisi çalışır; hata günlüğe yazılır", async () => {
    render(
      <>
        <ErrorBoundary name="Ekolayzer">
          <Fragile />
        </ErrorBoundary>
        <p>Sahne çalışıyor</p>
      </>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Ekolayzer açılamadı");
    expect(screen.getByText("Sahne çalışıyor")).toBeInTheDocument();
    await act(async () => undefined);
    expect(logged.messages).toHaveLength(1);
    expect(logged.messages[0]).toMatch(/^Ekolayzer çizilemedi: TypeError: Cannot read/);
    expect(logged.messages[0]).toMatch(/Bileşenler:\n.*Fragile/);

    // Sorun giderilince "Yeniden dene" bölümü geri getirir.
    state.broken = false;
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Yeniden dene" })));
    expect(screen.getByText("Panel çalışıyor")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("özel yedek görünüm verilebilir", () => {
    render(
      <ErrorBoundary name="Sahne" fallback={() => <p>Durgun gök</p>}>
        <Fragile />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Durgun gök")).toBeInTheDocument();
  });
});
