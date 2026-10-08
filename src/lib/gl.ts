/** Sahnelerin ortak WebGL2 yardımcıları: tam ekran üçgen ve gölgelendirici derleme. */

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

/** Tam ekran gölgelendirici programı; derlenemezse nedeni günlüğe yazılır ve `null` döner. */
export function linkFullscreenProgram(
  gl: WebGL2RenderingContext,
  fragmentSource: string,
  sceneName: string,
): WebGLProgram | null {
  const vertex = compile(gl, gl.VERTEX_SHADER, FULLSCREEN_VERTEX_SHADER, sceneName);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, fragmentSource, sceneName);
  const program = gl.createProgram();
  if (!vertex || !fragment || !program) return null;
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error(`${sceneName} gölgelendiricisi bağlanamadı:`, gl.getProgramInfoLog(program));
    gl.deleteProgram(program);
    return null;
  }
  return program;
}

function compile(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
  sceneName: string,
): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.error(`${sceneName} gölgelendiricisi derlenemedi:`, gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

/** Tuvali ekran boyuna getirir (en fazla 2× piksel yoğunluğu); piksel oranını döndürür. */
export function fitCanvas(canvas: HTMLCanvasElement): {
  width: number;
  height: number;
  pixel: number;
} {
  const pixel = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, Math.round(canvas.clientWidth * pixel));
  const height = Math.max(1, Math.round(canvas.clientHeight * pixel));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  return { width, height, pixel };
}
