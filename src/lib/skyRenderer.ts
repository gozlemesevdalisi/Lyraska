import {
  AURORA_BASE,
  BLOOM_LUMINANCE,
  LYRA_LINES,
  LYRA_STARS,
  SKY_THEMES,
  mixColors,
  parseHexColor,
  type SkyState,
} from "./sky";
import { fitCanvas, linkFullscreenProgram, openScene, retrySetup } from "./gl";

/**
 * "Gece göğü" sahnesinin WebGL2 çizimi. Tüm gök tek bir tam ekran
 * üçgende, parça gölgelendiricisinde hesaplanır: gökyüzü geçişi, yıldızlar,
 * Lyra takımyıldızı, kuzey ışıkları perdeleri ve uzak tepeler.
 *
 * Görsel Yönetmen'den gelenler: perde renkleri bölüm temasından (`--sky-theme-N-*`),
 * gerilimde perdeler ufka çekilip daralır, açılımda yükselip genişler, ölçü
 * başında perdenin boyunca bir dalga akar (yalnızca şekil; parlaklık değişmez).
 */
export interface SkyRenderer {
  draw(state: SkyState): void;
  dispose(): void;
}

type Rgb = [number, number, number];

/** Hata günlüğünde görünen sahne adı. */
const SCENE_NAME = "Gece göğü";

/** Renkler `:root` içindeki `--sky-*` değişkenlerinden okunur. */
const COLORS: { uniform: UniformName; variable: string; fallback: Rgb }[] = [
  { uniform: "uZenith", variable: "--sky-zenith", fallback: [0.01, 0.016, 0.043] },
  { uniform: "uHorizon", variable: "--sky-horizon", fallback: [0.04, 0.1, 0.16] },
  { uniform: "uStar", variable: "--sky-star", fallback: [0.91, 0.94, 1] },
  { uniform: "uGround", variable: "--sky-ground", fallback: [0.004, 0.008, 0.012] },
];

/** Tema renkleri okunamazsa: tema 0'ın renkleri. */
const AURORA_FALLBACK: { low: Rgb; high: Rgb } = {
  low: [0.27, 0.94, 0.65],
  high: [0.49, 0.36, 1],
};

/** Takımyıldızın ekrandaki yeri ve boyu (ekran yüksekliğine oranla). */
const LYRA_ANCHOR = { x: 0.84, y: 0.86 };
const LYRA_SCALE = 0.052; // derece başına

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

uniform vec2 uResolution;
uniform float uPixel;
uniform float uFlow;
uniform float uTwinkle;
uniform vec3 uEnergy;
uniform float uTension;
uniform float uBloom;
uniform float uRipple;
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
    // Ölçü başı dalgası: ekranın ortasından iki yana yayılıp söner.
    float spread = abs(p.x - 0.5 * aspect);
    float ripple = 0.018 * exp(-uRipple * 1.4) * sin(spread * 9.0 - uRipple * 7.0)
      * smoothstep(uRipple * 0.9 + 0.1, uRipple * 0.9 - 0.2, spread);
    float center = 0.5 + 0.08 * fi
      - 0.12 * (fbm(vec2(x * 0.7 - uFlow * 0.05 * (1.0 + 0.3 * fi), fi * 1.7)) - 0.5)
      + 0.05 * sin(x * 1.3 + uFlow * 0.11 * (1.0 + 0.2 * fi))
      - 0.12 * uTension + 0.05 * uBloom + ripple;
    float dy = uv.y - center;
    // Gerilimde perde daralır, açılımda yukarı doğru uzar.
    float tail = (0.16 + 0.04 * fi) * (1.0 - 0.45 * uTension) * (1.0 + 0.12 * uBloom);
    float shape = dy < 0.0 ? exp(-dy * dy / 0.0015) : exp(-dy / tail);
    // Perdenin dikey ışınları ve kıvrımları.
    float fold = fbm(vec2(x * 2.2 + uFlow * 0.07, fi * 3.1));
    float rays = 0.3 + 0.7 * smoothstep(0.25, 0.8, fbm(vec2(x * 11.0 + uFlow * 0.15 + fold * 2.0, uv.y * 0.5 - uFlow * 0.03)));
    float energy = i == 0 ? uEnergy.x : (i == 1 ? uEnergy.y : uEnergy.z);
    float strength = ${AURORA_BASE.toFixed(2)} + ${(1 - AURORA_BASE).toFixed(2)} * energy;
    float tint = clamp(dy * 3.0 + 0.2 + 0.2 * fi, 0.0, 1.0);
    aurora += mix(uAuroraLow, uAuroraHigh, tint) * shape * rays * strength * (1.0 - 0.22 * fi);
  }
  // Açılım: uzama (%12) ile birlikte toplam parlaklık payı BLOOM_LUMINANCE kadar.
  col += aurora * 0.55 * (1.0 + ${(BLOOM_LUMINANCE - 0.12).toFixed(2)} * uBloom);

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
 * `null` döner (sahne o zaman CSS ile çizilmiş durgun göğü gösterir); neden hata
 * günlüğüne yazılır.
 */
export function createSkyRenderer(canvas: HTMLCanvasElement): SkyRenderer | null {
  const opened = openScene(canvas, SCENE_NAME, (gl) => setup(gl, canvas));
  if (!opened) return null;
  const ctx = opened.gl;
  let resources: Resources | null = opened.resources;
  let lost = false;
  // Takımyıldız konumları yalnızca en-boy oranı değişince yeniden gönderilir.
  let lyraAspect = 0;

  const onLost = (event: Event) => {
    event.preventDefault(); // geri yüklenebilsin
    lost = true;
  };
  const onRestored = () => {
    resources = retrySetup(canvas, SCENE_NAME, () => setup(ctx, canvas));
    lost = resources === null;
    lyraAspect = 0;
  };
  canvas.addEventListener("webglcontextlost", onLost);
  canvas.addEventListener("webglcontextrestored", onRestored);

  return {
    draw(state) {
      if (lost || !resources) return;
      const { width, height, pixel } = fitCanvas(canvas);
      const { program, uniforms, vao } = resources;
      ctx.viewport(0, 0, width, height);
      ctx.useProgram(program);
      ctx.bindVertexArray(vao);
      ctx.uniform2f(uniforms.uResolution, width, height);
      ctx.uniform1f(uniforms.uPixel, pixel);
      ctx.uniform1f(uniforms.uFlow, state.flowTime);
      ctx.uniform1f(uniforms.uTwinkle, state.twinkleTime);
      ctx.uniform3f(uniforms.uEnergy, ...state.energies);
      ctx.uniform1f(uniforms.uTension, state.tension);
      ctx.uniform1f(uniforms.uBloom, state.bloom);
      ctx.uniform1f(uniforms.uRipple, state.rippleTime);
      const { from, to, mix } = state.palette;
      const a = resources.themes[from % resources.themes.length] ?? AURORA_FALLBACK;
      const b = resources.themes[to % resources.themes.length] ?? AURORA_FALLBACK;
      ctx.uniform3f(uniforms.uAuroraLow, ...mixColors(a.low, b.low, mix));
      ctx.uniform3f(uniforms.uAuroraHigh, ...mixColors(a.high, b.high, mix));
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

type UniformName =
  | "uResolution"
  | "uPixel"
  | "uFlow"
  | "uTwinkle"
  | "uEnergy"
  | "uTension"
  | "uBloom"
  | "uRipple"
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
  /** Bölüm temalarının perde renkleri (`--sky-theme-N-low/high`). */
  themes: { low: Rgb; high: Rgb }[];
}

/** Gölgelendiriciyi derler ve renkleri yükler; olmazsa nedeni içeren hata fırlatır. */
function setup(gl: WebGL2RenderingContext, canvas: HTMLCanvasElement): Resources {
  const program = linkFullscreenProgram(gl, FRAGMENT_SHADER);
  const vao = gl.createVertexArray();
  if (!vao) throw new Error("köşe dizisi oluşturulamadı");

  const location = (name: UniformName) => gl.getUniformLocation(program, name);
  const uniforms: Record<UniformName, WebGLUniformLocation | null> = {
    uResolution: location("uResolution"),
    uPixel: location("uPixel"),
    uFlow: location("uFlow"),
    uTwinkle: location("uTwinkle"),
    uEnergy: location("uEnergy"),
    uTension: location("uTension"),
    uBloom: location("uBloom"),
    uRipple: location("uRipple"),
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
  const themes = Array.from({ length: SKY_THEMES }, (_, i) => ({
    low: parseHexColor(style.getPropertyValue(`--sky-theme-${i}-low`), AURORA_FALLBACK.low),
    high: parseHexColor(style.getPropertyValue(`--sky-theme-${i}-high`), AURORA_FALLBACK.high),
  }));
  return { program, vao, uniforms, themes };
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
