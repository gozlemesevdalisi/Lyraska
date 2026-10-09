import {
  AURORA_BASE,
  BLOOM_LUMINANCE,
  LYRA_LINES,
  LYRA_STARS,
  SKY_THEMES,
  mixColors,
  parseHexColor,
  type SkyLook,
  type SkyState,
} from "./sky";
import { fitCanvas, linkFullscreenProgram, openScene, retrySetup } from "./gl";

/**
 * "Gece göğü" sahnesinin WebGL2 çizimi. Tüm gök tek bir tam ekran
 * üçgende, parça gölgelendiricisinde hesaplanır: gökyüzü geçişi, yıldızlar,
 * Lyra takımyıldızı (Vega'nın ışık çizgileriyle), kuzey ışıkları ve manzara.
 *
 * Kuzey ışıkları: yakından uzağa dört perde. Perdenin dikey kesiti pürüzsüzdür
 * (keskin ama kırılmasız alt kenar, üstte uzun ve soluk ışınlar); perde kendi
 * üstüne katlandığı yerde ışınları sıklaşır ve parlar. Renk altta temanın alt
 * renginden yukarıda üst rengine geçer. Manzara: göl (gök yansır), korona (ışınlar
 * tepedeki bir noktaya toplanır) ya da karlı vadi (çam ağaçları, perdelerle
 * renklenen kar).
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

/** Gölün kıyı çizgisi (ekran yüksekliğine oranla): altında gök yansır. */
const WATER_LINE = 0.24;

/** Takımyıldızın ekrandaki yeri ve boyu (ekran yüksekliğine oranla). */
const LYRA_ANCHOR = { x: 0.84, y: 0.86 };
const LYRA_SCALE = 0.052; // derece başına

/** Gölgelendiricide görünümün numarası (`#if LOOK == n`). */
const LOOK_NUMBER: Record<SkyLook, number> = { lake: 0, corona: 1, snow: 2 };

const fragmentShader = (look: SkyLook) => `#version 300 es
precision highp float;
#define LOOK ${LOOK_NUMBER[look]}

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

// Gölün kıyısı (ekran yüksekliğine oranla): altı su, üstü gök.
const float WATER = ${(look === "corona" ? 0.14 : WATER_LINE).toFixed(3)};

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float hash1(float n) {
  n = fract(n * 0.1031);
  n *= n + 33.33;
  n *= n + n;
  return fract(n);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Tek boyutlu yumuşak gürültü: perdenin boyunca değişenler (ışınlar, kıvrımlar) için.
float noise1(float x) {
  float i = floor(x);
  float f = fract(x);
  return mix(hash1(i), hash1(i + 1.0), f * f * (3.0 - 2.0 * f));
}

float fbm1(float x) {
  return 0.5 * noise1(x) + 0.28 * noise1(x * 2.13 + 7.1) + 0.14 * noise1(x * 4.37 + 3.3)
    + 0.08 * noise1(x * 8.91 + 1.7);
}

// Sivri tepeler için: gürültünün ortası tepe olur.
float ridged(float x) {
  float n = 1.0 - abs(2.0 * noise1(x) - 1.0);
  return n * n;
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

// Perdenin dikey kesiti (Chapman katmanı biçiminde): alt kenar keskin, tepe kenarın
// hemen üstünde ve yumuşak, yukarısı uzun ve soluk. Her yerde pürüzsüz: iki eğrinin
// birleştiği yerde kırılma (basamak) yoktur. u: kenardan yükseklik; tepe değeri 1.
float curtainProfile(float u, float sharp, float tail) {
  return exp(-u / tail - (sharp / tail) * (exp(min(-u / sharp, 30.0)) - 1.0));
}

// Kuzey ışıkları: dört perde, yakından uzağa. Yakındaki bası, ortadaki orta sesleri,
// uzaktaki tizi izler; en uzaktaki hepsinin ortalamasını. veil: perdelerin örtüsü (0..1).
vec3 aurora(vec2 uv, float aspect, out float veil) {
  vec3 sum = vec3(0.0);
  float x = uv.x * aspect;
  // Ölçü başı dalgası: ekranın ortasından iki yana yayılıp söner (yalnızca şekil).
  float spread = abs(x - 0.5 * aspect);
  float ripple = 0.016 * exp(-uRipple * 1.4) * sin(spread * 9.0 - uRipple * 7.0)
    * (1.0 - smoothstep(uRipple * 0.9 - 0.2, uRipple * 0.9 + 0.1, spread));
  for (int i = 0; i < 4; i++) {
    float fi = float(i);
    float depth = fi / 3.0;
    // Perdenin boyunca konum: uzaktakiler daha küçük görünür (sık kıvrım).
    float s = x * (1.0 + 0.7 * depth) + fi * 7.31;
    float t = uFlow * (1.0 - 0.3 * depth);
    // Perde katlanır: ekrandaki konumdan perdenin kendi boyundaki konuma (sw) çarpıtma.
    // Perdenin kendi üstüne kıvrıldığı yerde (dsw büyük) ışınlar sıklaşır ve parlar,
    // açıldığı yerde seyrelir: bakış doğrultusundaki ışık miktarı.
    float a1 = s * 1.9 + t * 0.09 + fi * 2.3;
    float a2 = s * 4.3 - t * 0.13 + fi * 1.1;
    float sw = s + 0.22 * sin(a1) + 0.12 * sin(a2);
    float dsw = 1.0 + 0.418 * cos(a1) + 0.516 * cos(a2);
    float fold = 0.3 + 0.7 * abs(dsw);
    // Alt kenar: kıvrılan bir şerit. Gerilimde ufka çekilip düzleşir, açılımda yükselir.
    float amp = (0.07 - 0.035 * depth) * (1.0 - 0.45 * uTension) * (1.0 + 0.3 * uBloom);
    // Büyük ölçekte perdeler eğik yaylar çizer: biri yükselirken öteki alçalır, kesişirler.
    float edge = 0.6 - 0.085 * fi + 0.07 * sin(x * 0.55 + 1.9 * fi + t * 0.012)
      + amp * (0.7 * sin(sw * 1.1 + t * 0.06) + 1.3 * (fbm1(sw * 1.7 + t * 0.05) - 0.5))
      - 0.12 * uTension + 0.06 * uBloom + ripple * (1.0 - 0.5 * depth);
    // Perdeler gökyüzünün her yerinde aynı değil: yer yer yoğunlaşır.
    float cluster = mix(0.05, 1.0, smoothstep(0.25, 0.72, noise1(sw * 0.38 + t * 0.015 + fi * 11.0)));
    // Işınlar (korona): perdenin boyunca ince dikey çizgiler; her ışının boyu ayrı.
    float drift = t * (0.22 + 0.08 * fi);
    float rays = smoothstep(0.3, 0.98, 0.5 * noise1(sw * 31.0 + drift)
      + 0.3 * noise1(sw * 77.0 - drift * 1.6 + 3.0) + 0.2 * noise1(sw * 13.0 + drift * 0.5));
    float reach = 0.45 + 1.1 * noise1(sw * 9.0 + t * 0.1 + 5.0 * fi);
    // Açılımda ışınlar %12 uzar (parlaklık payı sky.ts auroraLuminance ile aynı hesap).
    float tail = (0.2 - 0.06 * depth) * reach * (1.0 - 0.45 * uTension) * (1.0 + 0.12 * uBloom);
    float sharp = 0.0045 + 0.0055 * depth;
    float u = uv.y - edge;
    // Alt kenardaki parlak şerit perdenin boyunca süreklidir; ışınlar ondan yukarı uzanır.
    float band = curtainProfile(u, sharp, sharp * 7.0);
    float contrast = 0.9 - 0.4 * depth;
    float beams = curtainProfile(u - 0.01, sharp * 5.0, tail) * mix(1.0, 0.04 + 0.96 * rays * rays, contrast);
    float body = (0.75 * band + beams) * fold * cluster;
    // Renk katmanları: altta yeşil, yukarı çıktıkça mor; alt kenarda ince, açık bir şerit.
    float height = max(u, 0.0) / tail;
    vec3 tint = mix(uAuroraLow, uAuroraHigh, smoothstep(0.05, 0.9, height));
    float core = exp(-u * u / (sharp * sharp * 9.0));
    tint *= 1.0 + 0.45 * core;
    float energy = i == 0 ? uEnergy.x
      : (i == 1 ? uEnergy.y : (i == 2 ? uEnergy.z : dot(uEnergy, vec3(1.0 / 3.0))));
    float strength = (${AURORA_BASE.toFixed(2)} + ${(1 - AURORA_BASE).toFixed(2)} * energy)
      * (1.0 - 0.35 * depth);
    sum += tint * body * strength;
    // Hâle: perdenin çevresindeki geniş, yumuşak ışıma.
    float halo = exp(-(u - 0.05) * (u - 0.05) / 0.014) * cluster;
    sum += uAuroraLow * halo * 0.05 * strength;
  }
  // Açılım: uzama (%12) ile birlikte toplam parlaklık payı BLOOM_LUMINANCE kadar.
  sum *= 0.62 * (1.0 + ${(BLOOM_LUMINANCE - 0.12).toFixed(2)} * uBloom);
  veil = clamp(dot(sum, vec3(0.2126, 0.7152, 0.0722)) * 4.0, 0.0, 1.0);
  return sum;
}

// Gök: geçiş, yıldızlar, Lyra, kuzey ışıkları ve karşı kıyıdaki dağlar.
vec3 sky(vec2 uv, float aspect, float pxPerUnit) {
  vec2 p = vec2(uv.x * aspect, uv.y);
  vec3 col = mix(uHorizon, uZenith, smoothstep(WATER, 0.98, uv.y));
  // Ufukta soluk hava ışıması (airglow).
  col += uAuroraLow * 0.035 * exp(-(uv.y - WATER - 0.06) * (uv.y - WATER - 0.06) / 0.006);

  float veil;
#if LOOK == 1
  // Korona: perdeler başımızın üstünde; ışınlar ekranın üstündeki bir noktaya toplanır.
  vec2 fromTop = vec2(p.x - 0.5 * aspect, uv.y - 1.35);
  vec3 lights = aurora(vec2(atan(fromTop.x, -fromTop.y) + 0.5, 1.4 - length(fromTop)), 1.0, veil);
#else
  vec3 lights = aurora(uv, aspect, veil);
#endif

  // Yıldız alanı: hücre başına en fazla bir yıldız; parlak perdenin ardında solar.
  float cells = 26.0;
  vec2 g = p * cells;
  vec2 cell = floor(g);
  float h = hash(cell);
  if (h > 0.82) {
    vec2 offset = vec2(hash(cell + 3.1), hash(cell + 7.7)) - 0.5;
    float distPx = length(fract(g) - 0.5 - offset * 0.7) * pxPerUnit / cells;
    float twinkle = 0.78 + 0.22 * sin(uTwinkle * (0.8 + 1.6 * hash(cell + 5.0)) + h * 40.0);
    float brightness = (0.25 + 0.75 * hash(cell + 9.0)) * mix(1.0, twinkle, 0.4 + 0.6 * uEnergy.z);
    col += uStar * starGlow(distPx, mix(0.6, 1.3, hash(cell + 1.3)) * uPixel) * brightness * 0.8
      * (1.0 - 0.55 * veil);
  }

  // Lyra takımyıldızı: çizgiler müzikle hafifçe belirir; Vega'nın ince ışık çizgileri var.
  float line = 0.0;
${LYRA_LINES.map(
  ([a, b]) =>
    `  line = max(line, 1.0 - smoothstep(0.0, 1.2 * uPixel, segment(p, uLyra[${a}].xy, uLyra[${b}].xy) * pxPerUnit));`,
).join("\n")}
  col += uStar * line * (0.07 + 0.05 * dot(uEnergy, vec3(1.0 / 3.0)));
  for (int i = 0; i < ${LYRA_STARS.length}; i++) {
    vec2 d = (p - uLyra[i].xy) * pxPerUnit;
    float distPx = length(d);
    float b = uLyra[i].z;
    col += uStar * (starGlow(distPx, (0.9 + 1.4 * b) * uPixel) * (0.6 + 0.4 * b)
                  + starGlow(distPx, 7.0 * b * uPixel) * 0.12 * b);
    // Işık çizgileri (yalnızca en parlakta belirgin): yatay ve dikey ince izler.
    float reach = 26.0 * b * b * uPixel;
    float thin = 0.7 * uPixel;
    col += uStar * 0.22 * b * b * b
      * (exp(-abs(d.x) / reach) * exp(-d.y * d.y / (thin * thin))
       + exp(-abs(d.y) / reach) * exp(-d.x * d.x / (thin * thin)));
  }

  col += lights;

  // Karşı kıyıdaki dağlar: ışıkların önünde koyu siluet; kenarlarda yükselip sahneyi çerçeveler.
  float side = abs(uv.x - 0.5) * 2.0;
  float peaks = 0.6 * ridged(p.x * 1.2 + 4.0) + 0.3 * ridged(p.x * 3.1 + 1.0) + 0.1 * ridged(p.x * 8.3);
  float ridge = WATER + 0.004 + 0.13 * peaks * (0.35 + 0.65 * side * side) + 0.004 * noise1(p.x * 40.0);
  col = mix(col, uGround, smoothstep(ridge + 1.5 / pxPerUnit, ridge - 1.5 / pxPerUnit, uv.y));
#if LOOK == 2
  // Kar sınırında seyrek çam ağaçları: iki sıra, arkadaki daha küçük.
  for (int row = 0; row < 2; row++) {
    float width = row == 0 ? 0.04 : 0.024;
    float cellX = floor(p.x / width + 0.5 * float(row));
    float present = step(0.4, hash1(cellX * 1.7 + 3.0 + 11.0 * float(row)));
    float local = abs(fract(p.x / width + 0.5 * float(row)) - 0.5) * 2.0;
    float tall = (row == 0 ? 0.085 : 0.05) * (0.55 + 0.45 * hash1(cellX + 9.1)) * present;
    float y = (uv.y - WATER) / max(tall, 1e-4);
    // Çam: basamaklı dallar, uca doğru incelir.
    float crown = (1.0 - y) * (0.75 + 0.25 * fract(y * 4.0)) * 0.5;
    float tree = step(0.0, y) * smoothstep(crown + 0.04, crown - 0.04, local * 0.5);
    col = mix(col, uGround, tree * present * step(y, 1.0));
  }
#endif
  return col;
}

void main() {
  vec2 uv = gl_FragCoord.xy / uResolution;
  float aspect = uResolution.x / uResolution.y;
  float pxPerUnit = uResolution.y;

  vec3 col;
#if LOOK == 1
  col = sky(uv, aspect, pxPerUnit);
#elif LOOK == 2
  if (uv.y >= WATER) {
    col = sky(uv, aspect, pxPerUnit);
  } else {
    // Karlı vadi: kar gökyüzünün ve perdelerin ışığıyla renklenir. Kar dalgaları
    // uzağa doğru incelir (perspektif); yer yer küçük pırıltılar.
    float z = 0.05 / (WATER - uv.y + 0.004);
    vec2 ground = vec2((uv.x - 0.5) * aspect * z, z);
    float drift = 0.6 * noise(ground * vec2(2.2, 1.4)) + 0.4 * noise(ground * vec2(7.0, 4.0) + 3.0);
    float strength = ${AURORA_BASE.toFixed(2)} + ${(1 - AURORA_BASE).toFixed(2)} * dot(uEnergy, vec3(1.0 / 3.0));
    vec3 light = uHorizon * 0.9 + mix(uAuroraLow, uAuroraHigh, 0.2) * 0.22 * strength;
    col = vec3(0.8, 0.86, 0.96) * light * (0.45 + 0.85 * drift) * (0.75 + 0.45 * uv.y / WATER);
    vec2 grain = floor(gl_FragCoord.xy / max(uPixel, 1.0));
    float glint = step(0.997, hash(grain)) * (0.5 + 0.5 * sin(uTwinkle * 2.0 + hash(grain + 1.7) * 40.0));
    col += uStar * glint * 0.18 * (1.0 - 0.6 * (uv.y / WATER));
  }
#else
  if (uv.y >= WATER) {
    col = sky(uv, aspect, pxPerUnit);
  } else {
    // Göl: gök yansır. Yakındaki dalgacıklar büyük, kıyıya doğru incelir; yansıma
    // kıyıya yakın (sığ açıda) daha güçlüdür.
    float q = (WATER - uv.y) / WATER;
    float rows = uv.y * pxPerUnit / (1.0 + 5.0 * q);
    float wave = noise1(rows * 0.9 + uFlow * 0.6) - 0.5 + 0.5 * (noise1(rows * 2.3 - uFlow * 0.9) - 0.5);
    vec2 mirrored = vec2(uv.x + wave * (0.002 + 0.012 * q), 2.0 * WATER - uv.y);
    col = uGround + sky(mirrored, aspect, pxPerUnit) * mix(0.8, 0.4, q);
  }
#endif

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
export function createSkyRenderer(
  canvas: HTMLCanvasElement,
  look: SkyLook = "lake",
): SkyRenderer | null {
  const opened = openScene(canvas, SCENE_NAME, (gl) => setup(gl, canvas, look));
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
    resources = retrySetup(canvas, SCENE_NAME, () => setup(ctx, canvas, look));
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
function setup(gl: WebGL2RenderingContext, canvas: HTMLCanvasElement, look: SkyLook): Resources {
  const program = linkFullscreenProgram(gl, fragmentShader(look));
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
