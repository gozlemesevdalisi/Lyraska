import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SkyLook } from "../lib/sky";

const renderers = vi.hoisted(() => ({
  created: [] as {
    look: SkyLook;
    draw: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
  }[],
}));

vi.mock("../lib/skyRenderer", () => ({
  createSkyRenderer: (_canvas: HTMLCanvasElement, look: SkyLook) => {
    const renderer = { look, draw: vi.fn(), dispose: vi.fn() };
    renderers.created.push(renderer);
    return renderer;
  },
}));
vi.mock("../hooks/useVisualFeed", () => ({ useVisualFeed: () => undefined }));

import { SkyScene } from "./SkyScene";

describe("gece göğü sahnesi", () => {
  it("görünüm değişince gök yeni manzarayla kurulur ve hemen çizilir; eskisi bırakılır", () => {
    const { rerender, unmount } = render(<SkyScene playing={false} look="lake" />);
    expect(renderers.created.map((r) => r.look)).toEqual(["lake"]);
    expect(renderers.created[0]?.draw).toHaveBeenCalledTimes(1);

    rerender(<SkyScene playing={false} look="corona" />);
    expect(renderers.created.map((r) => r.look)).toEqual(["lake", "corona"]);
    expect(renderers.created[0]?.dispose).toHaveBeenCalledTimes(1);
    expect(renderers.created[1]?.draw).toHaveBeenCalledTimes(1);

    // Aynı görünümle yeniden çizimde gök baştan kurulmaz.
    rerender(<SkyScene playing={true} look="corona" />);
    expect(renderers.created).toHaveLength(2);
    unmount();
    expect(renderers.created[1]?.dispose).toHaveBeenCalledTimes(1);
  });
});
