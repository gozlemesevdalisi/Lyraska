/** Sahnelerin ortak WebGL2 yardımcıları: tam ekran üçgen, gölgelendirici derleme, hata bildirimi. */

import { logFrontendError } from "./backend";

/** Tam ekranı kaplayan tek üçgen (köşe verisi gerekmez). */
export const FULLSCREEN_VERTEX_SHADER = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

/** WebGL2 bağlamı; yoksa `null` (sahne o zaman CSS ile çizilmiş durgun görüntüyü gösterir). */
export function getWebGl2(canvas: HTMLCanvasElement): WebGL2RenderingContext | null {
  try {
    return canvas.getContext("webgl2", { antialias: false, alpha: false, depth: false });
  } catch {
    return null;
  }
}

/**
 * Tam ekran gölgelendirici programı. Derlenemez ya da bağlanamazsa nedeni (ekran
 * kartı sürücüsünün mesajı) içeren bir hata fırlatır.
 */
export function linkFullscreenProgram(
  gl: WebGL2RenderingContext,
  fragmentSource: string,
): WebGLProgram {
  const vertex = compile(gl, gl.VERTEX_SHADER, FULLSCREEN_VERTEX_SHADER, "köşe");
  const fragment = compile(gl, gl.FRAGMENT_SHADER, fragmentSource, "parça");
  const program = gl.createProgram();
  if (!program) throw new Error("gölgelendirici programı oluşturulamadı");
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program) ?? "";
    gl.deleteProgram(program);
    throw new Error(`gölgelendirici bağlanamadı: ${log.trim() || "neden bildirilmedi"}`);
  }
  return program;
}

function compile(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
  kind: string,
): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error(`${kind} gölgelendiricisi oluşturulamadı`);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "";
    gl.deleteShader(shader);
    throw new Error(`${kind} gölgelendiricisi derlenemedi: ${log.trim() || "neden bildirilmedi"}`);
  }
  return shader;
}

/** Ekran kartının adı (sürücünün bildirdiği; hata günlüğü ve denetim için). */
export function gpuName(gl: WebGL2RenderingContext): string {
  try {
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const name = (
      info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER)
    ) as unknown;
    return typeof name === "string" ? name : "bilinmiyor";
  } catch {
    return "bilinmiyor";
  }
}

/**
 * Sahne ekran kartında çizilemedi: nedeni tuvale işlenir (denetim için) ve hata
 * günlüğüne yazılır; kullanıcı "Hata günlüğü" ile bize iletebilir.
 */
export function reportSceneProblem(
  canvas: HTMLCanvasElement,
  sceneName: string,
  problem: string,
): void {
  canvas.dataset.webglError = problem;
  const message = `${sceneName} sahnesi ekran kartında çizilemedi: ${problem}`;
  console.error(message);
  logFrontendError(message).catch(() => {
    /* Günlüğe yazılamazsa yapılacak bir şey yok. */
  });
}

/**
 * Çiziciyi kurar; kurulum hata fırlatırsa nedeni bildirir ve `null` döner.
 * Başarıda ekran kartının adı tuvale işlenir (denetim için).
 */
export function openScene<T>(
  canvas: HTMLCanvasElement,
  sceneName: string,
  setup: (gl: WebGL2RenderingContext) => T,
): { gl: WebGL2RenderingContext; resources: T } | null {
  const gl = getWebGl2(canvas);
  if (!gl) {
    reportSceneProblem(canvas, sceneName, NO_WEBGL2);
    return null;
  }
  canvas.dataset.gpu = gpuName(gl);
  const resources = retrySetup(canvas, sceneName, () => setup(gl));
  return resources === null ? null : { gl, resources };
}

/** Kurulumu dener (ör. bağlam geri geldiğinde); hata olursa bildirir ve `null` döner. */
export function retrySetup<T>(
  canvas: HTMLCanvasElement,
  sceneName: string,
  setup: () => T,
): T | null {
  try {
    const resources = setup();
    delete canvas.dataset.webglError;
    return resources;
  } catch (error) {
    reportSceneProblem(canvas, sceneName, error instanceof Error ? error.message : String(error));
    return null;
  }
}

/** WebGL2 hiç açılamadığında günlüğe yazılan neden. */
export const NO_WEBGL2 = "WebGL2 açılamadı (ekran kartı sürücüsü desteklemiyor ya da engelliyor)";

/**
 * Sahne tuvalinin en fazla piksel sayısı (1080p). Sahneler tüm pencereyi kaplar; büyük ya
 * da yüksek yoğunluklu ekranda çizim çözünürlüğü bu sınıra indirilir, dahili ekran
 * kartında da her kare zamanında çizilir. Görüntü yumuşak olduğundan fark edilmez.
 */
export const MAX_CANVAS_PIXELS = 1920 * 1080;

/**
 * Tuvalin piksel oranı: ekranın piksel yoğunluğu (en fazla 2×), tuval `MAX_CANVAS_PIXELS`'ı
 * aşacaksa daha düşük (en az 0,5).
 */
export function canvasScale(cssWidth: number, cssHeight: number, devicePixelRatio: number): number {
  const pixel = Math.min(devicePixelRatio || 1, 2);
  const area = Math.max(1, cssWidth) * Math.max(1, cssHeight);
  return Math.max(0.5, Math.min(pixel, Math.sqrt(MAX_CANVAS_PIXELS / area)));
}

/** Tuvali ekran boyuna getirir (bkz. `canvasScale`); piksel oranını döndürür. */
export function fitCanvas(canvas: HTMLCanvasElement): {
  width: number;
  height: number;
  pixel: number;
} {
  const pixel = canvasScale(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio);
  const width = Math.max(1, Math.round(canvas.clientWidth * pixel));
  const height = Math.max(1, Math.round(canvas.clientHeight * pixel));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  return { width, height, pixel };
}
