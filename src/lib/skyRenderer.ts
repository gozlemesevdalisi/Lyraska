import { LYRA_LINES, LYRA_STARS, AURORA_BASE, parseHexColor, type SkyState } from "./sky";

/**
 * "Gece göğü" sahnesinin WebGL2 çizimi. Tüm gök tek bir tam ekran
 * üçgende, parça gölgelendiricisinde hesaplanır: gökyüzü geçişi, yıldızlar,
 * Lyra takımyıldızı, kuzey ışıkları perdeleri ve uzak tepeler.
 */
export interface SkyRenderer {
  draw(state: SkyState): void;
  dispose(): void;
}

type Rgb = [number, number, number];

/** Renkler `:root` içindeki `--sky-*` değişkenlerinden okunur. */
const COLORS: { uniform: UniformName; variable: string; fallback: Rgb }[] = [
  { uniform: "uZenith", variable: "--sky-zenith", fallback: [0.01, 0.016, 0.043] },
  { uniform: "uHorizon", variable: "--sky-horizon", fallback: [0.04, 0.1, 0.16] },
  { uniform: "uAuroraLow", variable: "--sky-aurora-low", fallback: [0.29, 0.94, 0.63] },
  { uniform: "uAuroraHigh", variable: "--sky-aurora-high", fallback: [0.48, 0.36, 1] },
  { uniform: "uStar", variable: "--sky-star", fallback: [0.91, 0.94, 1] },
  { uniform: "uGround", variable: "--sky-ground", fallback: [0.004, 0.008, 0.012] },
];

/** Takımyıldızın ekrandaki yeri ve boyu (ekran yüksekliğine oranla). */
const LYRA_ANCHOR = { x: 0.84, y: 0.86 };
const LYRA_SCALE = 0.052; // derece başına

const VERTEX_SHADER = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

uniform vec2 uResolution;
uniform float uPixel;
uniform float uFlow;
uniform float uTwinkle;
uniform vec3 uEnergy;
uniform vec3 uZenith;
uniform vec3 uHorizon;
uniform vec3 uAuroraLow;
uniform vec3 uAuroraHigh;
uniform vec3 uStar;
uniform vec3 uGround;
uniform vec3 uLyra[${LYRA_STARS.length}];

out vec4 outColor;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 5; i++) {
    v += a * noise(p);
    p = p * 2.03 + vec2(17.1, 9.2);
    a *= 0.5;
  }
  return v;
}

float segment(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a;
  vec2 ba = b - a;
  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

// Yarıçapı piksel cinsinden yumuşak bir yıldız ışığı.
float starGlow(float distPx, float radiusPx) {
  return exp(-distPx * distPx / (radiusPx * radiusPx));
}

void main() {
  vec2 uv = gl_FragCoord.xy / uResolution;
  float aspect = uResolution.x / uResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float pxPerUnit = uResolution.y;

  vec3 col = mix(uHorizon, uZenith, smoothstep(0.05, 0.95, uv.y));

  // Yıldız alanı: hücre başına en fazla bir yıldız.
  float cells = 26.0;
  vec2 g = p * cells;
  vec2 cell = floor(g);
  float h = hash(cell);
  if (h > 0.82) {
    vec2 offset = vec2(hash(cell + 3.1), hash(cell + 7.7)) - 0.5;
    float distPx = length(fract(g) - 0.5 - offset * 0.7) * pxPerUnit / cells;
    float twinkle = 0.78 + 0.22 * sin(uTwinkle * (0.8 + 1.6 * hash(cell + 5.0)) + h * 40.0);
    float brightness = (0.25 + 0.75 * hash(cell + 9.0)) * mix(1.0, twinkle, 0.4 + 0.6 * uEnergy.z);
    col += uStar * starGlow(distPx, mix(0.6, 1.3, hash(cell + 1.3)) * uPixel) * brightness * 0.8;
  }

  // Lyra takımyıldızı: soluk çizgiler ve yıldızlar.
  float line = 0.0;
${LYRA_LINES.map(
  ([a, b]) =>
    `  line = max(line, 1.0 - smoothstep(0.0, 1.2 * uPixel, segment(p, uLyra[${a}].xy, uLyra[${b}].xy) * pxPerUnit));`,
).join("\n")}
  col += uStar * line * 0.1;
  for (int i = 0; i < ${LYRA_STARS.length}; i++) {
    float distPx = length(p - uLyra[i].xy) * pxPerUnit;
    float b = uLyra[i].z;
    col += uStar * (starGlow(distPx, (0.9 + 1.4 * b) * uPixel) * (0.6 + 0.4 * b)
                  + starGlow(distPx, 7.0 * b * uPixel) * 0.12 * b);
  }

  // Kuzey ışıkları: bas, orta ve tiz için üç perde. Alt kenarı keskin, üstü uzun ve soluk.
  vec3 aurora = vec3(0.0);
  for (int i = 0; i < 3; i++) {
    float fi = float(i);
    float x = p.x * (0.85 + 0.2 * fi) + fi * 3.7;
    float center = 0.5 + 0.08 * fi
      - 0.12 * (fbm(vec2(x * 0.7 - uFlow * 0.05 * (1.0 + 0.3 * fi), fi * 1.7)) - 0.5)
      + 0.05 * sin(x * 1.3 + uFlow * 0.11 * (1.0 + 0.2 * fi));
    float dy = uv.y - center;
    float shape = dy < 0.0 ? exp(-dy * dy / 0.0015) : exp(-dy / (0.16 + 0.04 * fi));
    // Perdenin dikey ışınları ve kıvrımları.
    float fold = fbm(vec2(x * 2.2 + uFlow * 0.07, fi * 3.1));
    float rays = 0.3 + 0.7 * smoothstep(0.25, 0.8, fbm(vec2(x * 11.0 + uFlow * 0.15 + fold * 2.0, uv.y * 0.5 - uFlow * 0.03)));
    float energy = i == 0 ? uEnergy.x : (i == 1 ? uEnergy.y : uEnergy.z);
    float strength = ${AURORA_BASE.toFixed(2)} + ${(1 - AURORA_BASE).toFixed(2)} * energy;
    float tint = clamp(dy * 3.0 + 0.2 + 0.2 * fi, 0.0, 1.0);
    aurora += mix(uAuroraLow, uAuroraHigh, tint) * shape * rays * strength * (1.0 - 0.22 * fi);
  }
  col += aurora * 0.55;

  // Uzak tepeler: ışıkların önünde koyu siluet.
  float ridge = 0.04 + 0.16 * fbm(vec2(p.x * 1.1, 2.0)) + 0.03 * fbm(vec2(p.x * 6.0, 5.0));
  col = mix(col, uGround, smoothstep(ridge + 1.5 / pxPerUnit, ridge - 1.5 / pxPerUnit, uv.y));

  // Kenar kararması ve renk kuşaklanmasına karşı titreşim (dither).
  col *= 1.0 - 0.3 * pow(length(uv - 0.5) * 1.2, 2.0);
  col += (hash(gl_FragCoord.xy + fract(uTwinkle)) - 0.5) / 255.0;
  outColor = vec4(col, 1.0);
}`;

/**
 * Tuvale gök çizicisini kurar. WebGL2 yoksa ya da gölgelendirici derlenemezse
 * `null` döner (sahne o zaman CSS ile çizilmiş durgun göğü gösterir).
 */
export function createSkyRenderer(canvas: HTMLCanvasElement): SkyRenderer | null {
  const ctx = getContext(canvas);
  if (!ctx) return null;

  let resources = setup(ctx, canvas);
  if (!resources) return null;
  let lost = false;
  // Takımyıldız konumları yalnızca en-boy oranı değişince yeniden gönderilir.
  let lyraAspect = 0;

  const onLost = (event: Event) => {
    event.preventDefault(); // geri yüklenebilsin
    lost = true;
  };
  const onRestored = () => {
    resources = setup(ctx, canvas);
    lost = resources === null;
    lyraAspect = 0;
  };
  canvas.addEventListener("webglcontextlost", onLost);
  canvas.addEventListener("webglcontextrestored", onRestored);

  return {
    draw(state) {
      if (lost || !resources) return;
      const pixel = Math.min(window.devicePixelRatio || 1, 2);
      const width = Math.max(1, Math.round(canvas.clientWidth * pixel));
      const height = Math.max(1, Math.round(canvas.clientHeight * pixel));
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      const { program, uniforms, vao } = resources;
      ctx.viewport(0, 0, width, height);
      ctx.useProgram(program);
      ctx.bindVertexArray(vao);
      ctx.uniform2f(uniforms.uResolution, width, height);
      ctx.uniform1f(uniforms.uPixel, pixel);
      ctx.uniform1f(uniforms.uFlow, state.flowTime);
      ctx.uniform1f(uniforms.uTwinkle, state.twinkleTime);
      ctx.uniform3f(uniforms.uEnergy, ...state.energies);
      if (lyraAspect !== width / height) {
        lyraAspect = width / height;
        ctx.uniform3fv(uniforms.uLyra, lyraPositions(lyraAspect));
      }
      ctx.drawArrays(ctx.TRIANGLES, 0, 3);
    },
    dispose() {
      canvas.removeEventListener("webglcontextlost", onLost);
      canvas.removeEventListener("webglcontextrestored", onRestored);
      if (resources && !ctx.isContextLost()) {
        ctx.deleteProgram(resources.program);
        ctx.deleteVertexArray(resources.vao);
      }
      resources = null;
    },
  };
}

function getContext(canvas: HTMLCanvasElement): WebGL2RenderingContext | null {
  try {
    return canvas.getContext("webgl2", { antialias: false, alpha: false, depth: false });
  } catch {
    return null;
  }
}

type UniformName =
  | "uResolution"
  | "uPixel"
  | "uFlow"
  | "uTwinkle"
  | "uEnergy"
  | "uLyra"
  | "uZenith"
  | "uHorizon"
  | "uAuroraLow"
  | "uAuroraHigh"
  | "uStar"
  | "uGround";

interface Resources {
  program: WebGLProgram;
  vao: WebGLVertexArrayObject;
  uniforms: Record<UniformName, WebGLUniformLocation | null>;
}

function setup(gl: WebGL2RenderingContext, canvas: HTMLCanvasElement): Resources | null {
  const program = linkProgram(gl);
  const vao = gl.createVertexArray();
  if (!program || !vao) return null;

  const location = (name: UniformName) => gl.getUniformLocation(program, name);
  const uniforms: Record<UniformName, WebGLUniformLocation | null> = {
    uResolution: location("uResolution"),
    uPixel: location("uPixel"),
    uFlow: location("uFlow"),
    uTwinkle: location("uTwinkle"),
    uEnergy: location("uEnergy"),
    uLyra: location("uLyra"),
    uZenith: location("uZenith"),
    uHorizon: location("uHorizon"),
    uAuroraLow: location("uAuroraLow"),
    uAuroraHigh: location("uAuroraHigh"),
    uStar: location("uStar"),
    uGround: location("uGround"),
  };

  gl.useProgram(program);
  const style = window.getComputedStyle(canvas);
  for (const { uniform, variable, fallback } of COLORS) {
    gl.uniform3f(uniforms[uniform], ...parseHexColor(style.getPropertyValue(variable), fallback));
  }
  return { program, vao, uniforms };
}

function linkProgram(gl: WebGL2RenderingContext): WebGLProgram | null {
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
  const program = gl.createProgram();
  if (!vertex || !fragment || !program) return null;
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error("Gece göğü gölgelendiricisi bağlanamadı:", gl.getProgramInfoLog(program));
    gl.deleteProgram(program);
    return null;
  }
  return program;
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.error("Gece göğü gölgelendiricisi derlenemedi:", gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

/** Takımyıldızın gölgelendiriciye giden konumları (x, y ekran birimi; z parlaklık). */
export function lyraPositions(aspect: number): Float32Array {
  const out = new Float32Array(LYRA_STARS.length * 3);
  LYRA_STARS.forEach((star, i) => {
    out[i * 3] = LYRA_ANCHOR.x * aspect + star.x * LYRA_SCALE;
    out[i * 3 + 1] = LYRA_ANCHOR.y + star.y * LYRA_SCALE;
    out[i * 3 + 2] = star.brightness;
  });
  return out;
}
