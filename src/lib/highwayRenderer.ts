import { fitCanvas, getWebGl2, linkFullscreenProgram } from "./gl";
import { BLOOM_LUMINANCE, SKY_THEMES, mixColors, parseHexColor } from "./sky";
import { DASHES_PER_LAMP, GLOW_BASE, type HighwayState } from "./highway";

/**
 * "Gece otoyolu" sahnesinin WebGL2 çizimi. Tek tam ekran üçgende, parça
 * gölgelendiricisinde: gece göğü, ufuk parıltısı, şehir silueti ve pencereleri,
 * perspektifli yol (şerit çizgileri, kenar çizgileri, sis) ve sokak lambaları.
 *
 * Lambalar kameraya yaklaşınca söner: ekranın büyük bir bölümünü kaplayıp
 * ölçü başlarında parlama üretmezler.
 */
export interface HighwayRenderer {
  draw(state: HighwayState): void;
  dispose(): void;
}

type Rgb = [number, number, number];

/** Renkler `:root` içindeki `--road-*` değişkenlerinden okunur. */
const COLORS: { uniform: UniformName; variable: string; fallback: Rgb }[] = [
  { uniform: "uSkyTop", variable: "--road-sky-top", fallback: [0.012, 0.012, 0.04] },
  { uniform: "uSkyBottom", variable: "--road-sky-bottom", fallback: [0.05, 0.043, 0.118] },
  { uniform: "uAsphalt", variable: "--road-asphalt", fallback: [0.043, 0.047, 0.063] },
  { uniform: "uLine", variable: "--road-line", fallback: [0.79, 0.8, 0.84] },
  { uniform: "uLampColor", variable: "--road-lamp", fallback: [1, 0.77, 0.43] },
  { uniform: "uWindow", variable: "--road-window", fallback: [1, 0.81, 0.48] },
  { uniform: "uCity", variable: "--road-city", fallback: [0.016, 0.016, 0.035] },
  { uniform: "uGround", variable: "--road-ground", fallback: [0.02, 0.02, 0.03] },
];

/** Tema renkleri okunamazsa: tema 0'ın renkleri. */
const GLOW_FALLBACK: { low: Rgb; high: Rgb } = {
  low: [0.27, 0.94, 0.65],
  high: [0.49, 0.36, 1],
};

/** Çizgi aralığı (dünya birimi); lamba aralığı bunun `DASHES_PER_LAMP` katı. */
const DASH_PERIOD = 6;
const LAMPS = 7;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

uniform vec2 uResolution;
uniform float uPixel;
uniform float uDash;
uniform float uLamp;
uniform float uGlow;
uniform float uTension;
uniform float uBloom;
uniform float uTime;
uniform vec3 uGlowLow;
uniform vec3 uGlowHigh;
uniform vec3 uSkyTop;
uniform vec3 uSkyBottom;
uniform vec3 uAsphalt;
uniform vec3 uLine;
uniform vec3 uLampColor;
uniform vec3 uWindow;
uniform vec3 uCity;
uniform vec3 uGround;

out vec4 outColor;

const float HORIZON = 0.44;   // ufkun ekrandaki yüksekliği
const float CAMERA = 0.55;    // kameranın yerden yüksekliği (dünya birimi)
const float ROAD = 1.6;       // yolun yarı genişliği
const float DASH_PERIOD = ${DASH_PERIOD.toFixed(1)};
const float LAMP_PERIOD = ${(DASH_PERIOD * DASHES_PER_LAMP).toFixed(1)};
const float LAMP_SIDE = 2.4;  // lambaların yolun ortasına uzaklığı
const float LAMP_HEIGHT = 1.7;
const float FOG = 0.022;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

// Yolun uzakta hafifçe kıvrılması: derinliğe göre yanal kayma (dünya birimi).
float curve(float z) {
  float bend = 0.55 * sin(uTime * 0.05) + 0.25 * sin(uTime * 0.013 + 1.7);
  return bend * z * z * 0.004;
}

void main() {
  vec2 uv = gl_FragCoord.xy / uResolution;
  float aspect = uResolution.x / uResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float vx = 0.5 * aspect;
  float px = 1.0 / uResolution.y;

  // Ufuk parıltısı: rengi bölüm temasından; gerilimde ufka çekilir, açılımda yükselir.
  float strength = (${GLOW_BASE.toFixed(2)} + ${(1 - GLOW_BASE).toFixed(2)} * uGlow)
    * (1.0 + ${(BLOOM_LUMINANCE * 0.4).toFixed(2)} * uBloom);
  // Açılımda parıltı yükselir (alanı büyür); tepe parlaklığı az artar.
  float height = 0.15 * (1.0 - 0.45 * uTension) * (1.0 + 0.4 * uBloom);
  float width = 1.0 * (1.0 - 0.3 * uTension) * (1.0 + 0.1 * uBloom);
  float dx = p.x - vx;
  float across = exp(-dx * dx / (width * width));

  vec3 col;
  if (uv.y >= HORIZON) {
    float dy = uv.y - HORIZON;
    col = mix(uSkyBottom, uSkyTop, smoothstep(0.0, 0.56, dy));
    // Soluk yıldızlar.
    vec2 g = p * 70.0;
    vec2 cell = floor(g);
    float h = hash(cell);
    if (h > 0.965) {
      vec2 offset = vec2(hash(cell + 3.1), hash(cell + 7.7)) - 0.5;
      float d = length(fract(g) - 0.5 - offset * 0.6) * uResolution.y / 70.0;
      col += vec3(0.8, 0.85, 1.0) * exp(-d * d / (uPixel * uPixel)) * 0.3 * hash(cell + 9.0)
        * smoothstep(0.08, 0.3, dy);
    }
    vec3 tint = mix(uGlowLow, uGlowHigh, smoothstep(0.0, height * 2.5, dy));
    col += tint * exp(-dy / height) * across * strength * 0.9;

    // Şehir silueti: ortada yüksek, kenarlara doğru alçalan binalar.
    float bw = 0.035;
    float bx = floor(p.x / bw);
    float mask = exp(-dx * dx / 0.9);
    float top = (0.008 + 0.06 * pow(hash(vec2(bx, 3.0)), 2.0)) * mask;
    if (dy < top) {
      col = uCity;
      // Pencereler: çoğu karanlık; arada bir yanar ya da söner (yavaş).
      vec2 w = vec2(p.x / 0.0055, dy / 0.0075);
      vec2 wc = floor(w);
      vec2 wf = fract(w);
      float slot = floor(uTime / 9.0 + hash(wc) * 9.0);
      float lit = step(0.8, hash(wc + slot * 0.37));
      float inside = step(0.2, wf.x) * step(wf.x, 0.75) * step(0.25, wf.y) * step(wf.y, 0.8);
      float edge = step(bw * 0.1, mod(p.x, bw)) * step(dy, top - 0.004);
      col += uWindow * lit * inside * edge * 0.4;
    }
  } else {
    // Yer: perspektif. z = derinlik, x = yolun ortasına göre yanal konum.
    float z = min(CAMERA / max(HORIZON - uv.y, 1e-4), 400.0);
    float x = dx * z - curve(z);
    float aa = max(fwidth(x), 1e-4);
    float fog = exp(-z * FOG);

    vec3 ground = uGround;
    if (abs(x) < ROAD) {
      ground = uAsphalt * (0.85 + 0.3 * hash(floor(vec2(x * 40.0, z * 8.0))));
      // Kenar çizgileri ve ortadaki kesikli şerit çizgisi.
      float edgeLine = smoothstep(aa, 0.0, abs(abs(x) - ROAD * 0.92) - 0.035);
      float dash = step(fract(z / DASH_PERIOD + uDash), 0.45);
      float center = smoothstep(aa, 0.0, abs(x) - 0.05) * dash;
      ground = mix(ground, uLine * 0.45, max(edgeLine, center));
    }
    // Islak yolda ufuk parıltısının yansıması.
    vec3 tint = mix(uGlowLow, uGlowHigh, 0.3);
    ground += tint * across * strength * 0.18 * (1.0 - fog);

    // Lamba ışığının yola düştüğü yer.
    for (int k = 0; k < ${LAMPS}; k++) {
      float lz = LAMP_PERIOD * (float(k) + 1.0 - fract(uLamp));
      float near = smoothstep(2.0, 7.0, lz);
      float dz = z - lz;
      float side = min(abs(x - LAMP_SIDE + 0.6), abs(x + LAMP_SIDE - 0.6));
      ground += uLampColor * 0.1 * near * exp(-lz * FOG) * exp(-side * side / 1.5 - dz * dz / 12.0);
    }
    col = mix(uSkyBottom * 0.8, ground, fog);
  }

  // Sokak lambaları: direk ve ışık; uzakta sise, yakında sönerek kaybolur.
  for (int k = 0; k < ${LAMPS}; k++) {
    float lz = LAMP_PERIOD * (float(k) + 1.0 - fract(uLamp));
    float near = smoothstep(2.0, 7.0, lz);
    float fade = exp(-lz * FOG) * near;
    float baseY = HORIZON - CAMERA / lz;
    float headY = HORIZON + (LAMP_HEIGHT - CAMERA) / lz;
    for (int s = 0; s < 2; s++) {
      float sx = s == 0 ? -1.0 : 1.0;
      float lx = vx + (sx * LAMP_SIDE + curve(lz)) / lz;
      float post = max(0.025 / lz, px * 0.8);
      if (abs(p.x - lx) < post && uv.y > baseY && uv.y < headY) {
        col = mix(col, uLine * 0.1, fade);
      }
      vec2 head = vec2(lx - sx * 0.35 / lz, headY);
      // Lambanın kolu: direkten yola doğru.
      if (abs(uv.y - headY) < max(0.02 / lz, px * 0.6) && (p.x - lx) * sx < 0.0
          && (p.x - lx) * sx > -0.35 / lz) {
        col = mix(col, uLine * 0.1, fade);
      }
      float d = length(p - head) * lz;
      col += uLampColor * fade * (exp(-d * d / 0.012) * 0.9 + exp(-d * d / 0.35) * 0.1);
    }
  }

  // Kenar kararması ve renk kuşaklanmasına karşı titreşim (dither).
  col *= 1.0 - 0.3 * pow(length(uv - 0.5) * 1.2, 2.0);
  col += (hash(gl_FragCoord.xy + fract(uTime)) - 0.5) / 255.0;
  outColor = vec4(col, 1.0);
}`;

/**
 * Tuvale otoyol çizicisini kurar. WebGL2 yoksa ya da gölgelendirici derlenemezse
 * `null` döner (sahne o zaman CSS ile çizilmiş durgun görüntüyü gösterir).
 */
export function createHighwayRenderer(canvas: HTMLCanvasElement): HighwayRenderer | null {
  const ctx = getWebGl2(canvas);
  if (!ctx) return null;

  let resources = setup(ctx, canvas);
  if (!resources) return null;
  let lost = false;

  const onLost = (event: Event) => {
    event.preventDefault(); // geri yüklenebilsin
    lost = true;
  };
  const onRestored = () => {
    resources = setup(ctx, canvas);
    lost = resources === null;
  };
  canvas.addEventListener("webglcontextlost", onLost);
  canvas.addEventListener("webglcontextrestored", onRestored);

  return {
    draw(state) {
      if (lost || !resources) return;
      const { width, height, pixel } = fitCanvas(canvas);
      const { program, uniforms, vao, themes } = resources;
      ctx.viewport(0, 0, width, height);
      ctx.useProgram(program);
      ctx.bindVertexArray(vao);
      ctx.uniform2f(uniforms.uResolution, width, height);
      ctx.uniform1f(uniforms.uPixel, pixel);
      ctx.uniform1f(uniforms.uDash, state.dash % 1);
      ctx.uniform1f(uniforms.uLamp, state.lamp % 1);
      ctx.uniform1f(uniforms.uGlow, state.glow);
      ctx.uniform1f(uniforms.uTension, state.tension);
      ctx.uniform1f(uniforms.uBloom, state.bloom);
      ctx.uniform1f(uniforms.uTime, state.time);
      const { from, to, mix } = state.palette;
      const a = themes[from % themes.length] ?? GLOW_FALLBACK;
      const b = themes[to % themes.length] ?? GLOW_FALLBACK;
      ctx.uniform3f(uniforms.uGlowLow, ...mixColors(a.low, b.low, mix));
      ctx.uniform3f(uniforms.uGlowHigh, ...mixColors(a.high, b.high, mix));
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
  | "uDash"
  | "uLamp"
  | "uGlow"
  | "uTension"
  | "uBloom"
  | "uTime"
  | "uGlowLow"
  | "uGlowHigh"
  | "uSkyTop"
  | "uSkyBottom"
  | "uAsphalt"
  | "uLine"
  | "uLampColor"
  | "uWindow"
  | "uCity"
  | "uGround";

const UNIFORMS: UniformName[] = [
  "uResolution",
  "uPixel",
  "uDash",
  "uLamp",
  "uGlow",
  "uTension",
  "uBloom",
  "uTime",
  "uGlowLow",
  "uGlowHigh",
  "uSkyTop",
  "uSkyBottom",
  "uAsphalt",
  "uLine",
  "uLampColor",
  "uWindow",
  "uCity",
  "uGround",
];

interface Resources {
  program: WebGLProgram;
  vao: WebGLVertexArrayObject;
  uniforms: Record<UniformName, WebGLUniformLocation | null>;
  /** Bölüm temalarının ufuk renkleri (gece göğüyle aynı `--sky-theme-N-*`). */
  themes: { low: Rgb; high: Rgb }[];
}

function setup(gl: WebGL2RenderingContext, canvas: HTMLCanvasElement): Resources | null {
  const program = linkFullscreenProgram(gl, FRAGMENT_SHADER, "Gece otoyolu");
  const vao = gl.createVertexArray();
  if (!program || !vao) return null;

  const uniforms = Object.fromEntries(
    UNIFORMS.map((name) => [name, gl.getUniformLocation(program, name)]),
  ) as Record<UniformName, WebGLUniformLocation | null>;

  gl.useProgram(program);
  const style = window.getComputedStyle(canvas);
  for (const { uniform, variable, fallback } of COLORS) {
    gl.uniform3f(uniforms[uniform], ...parseHexColor(style.getPropertyValue(variable), fallback));
  }
  const themes = Array.from({ length: SKY_THEMES }, (_, i) => ({
    low: parseHexColor(style.getPropertyValue(`--sky-theme-${i}-low`), GLOW_FALLBACK.low),
    high: parseHexColor(style.getPropertyValue(`--sky-theme-${i}-high`), GLOW_FALLBACK.high),
  }));
  return { program, vao, uniforms, themes };
}
