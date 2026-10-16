//! Crossfeed: kulaklıkta hoparlörden dinler gibi.
//!
//! Hoparlörle dinlerken her kulak iki hoparlörü de duyar: karşı hoparlörün sesi başın
//! gölgesinde biraz kısılmış ve ~0,3 ms gecikmiş gelir. Kulaklıkta bu karışma yoktur; sola
//! kaydırılmış bir çalgı yalnızca sol kulakta çalar, ses "kafanın içinde" ve yorucu olur.
//! Crossfeed, karşı kanalı süzüp geciktirerek karıştırır.
//!
//! Sıradan crossfeed'lerin (Bauer 1961, Linkwitz, Chu Moy, Meier, bs2b) bilinen kusuru ton
//! rengidir: ortadaki ses (vokal, bas, davul) iki yoldan toplanır ve frekans yanıtı bozulur.
//! Lyraska'daki tasarım bunu matematiksel olarak ortadan kaldırır.
//!
//! **Orta/yan biçimi.** M = (L + R)/2, S = (L − R)/2. Çıkış L' = M + G·S, R' = M − G·S. Orta
//! (mono) bileşene hiç dokunulmaz: ortadaki her ses bit bit aynı çalar. Yalnızca yan bileşen
//! G(z) süzgecinden geçer.
//!
//! **G'nin seçimi.** Tamamen sola kaydırılmış bir seste (R = 0) iki kulağa gidenlerin oranı
//! R'/L' = (1 − G)/(1 + G) = ρ olur; ρ, karşı kulağın aynı kulağa göre aldığı paydır. Bunun
//! tersi G = (1 − ρ)/(1 + ρ). Hedef, birinci dereceden alçak geçiren ρ(s) = r0/(1 + s/ωc):
//! - alçak frekansta kulaklar arası seviye farkı −20·log₁₀(r0) dB,
//! - zaman farkı (grup gecikmesi) 1/ωc (700 Hz'te ~0,23 ms; baş modelinde ±30° için ~0,28 ms),
//! - fazı hiçbir frekansta 90°'yi geçmez, yani Re ρ ≥ 0. Bu, |G| ≤ 1 demektir: yan bileşen
//!   hiçbir frekansta yükselmez, tepe ya da rezonans oluşmaz, ses taşmaz.
//!
//! Yerine koyunca G(s) = (s + ωc·(1 − r0)) / (s + ωc·(1 + r0)): tek kutuplu bir raf. DC'de
//! (1 − r0)/(1 + r0) (yan bileşenin bası kısılır: bas iki kulağa da gider), yüksek frekansta 1
//! (kulaklık zaten doğru). Sayısal karşılığı çift doğrusal dönüşümle bulunur: |G| ≤ 1 özelliği
//! ve Nyquist'te tam 1 olması korunur, kulaklar arası oran ρ'nun aynı dönüşümüdür.
//!
//! Faz 90° ile sınırlı olduğu için ~600 Hz'in üstünde karşı kulağın payı gerçek baştan biraz
//! hızlı azalır (gerçek başta karşı ses orada tarak süzgeci gibi ton rengi yaratır); bu bilerek
//! seçilmiş bir ödünleşimdir: ses rengi yerine doğal sahne. Hazır ayarlar bs2b'nin bilinen
//! değerlerinin (kesim frekansı, seviye farkı) karşılığıdır; "Hoparlör gibi" küresel baş
//! modeline (Brown ve Duda 1998, ±30°) birinci dereceden en iyi uyumdur.
//!
//! Gerçek zamanlı yolda bellek ayırma ve kilit yoktur; ayar değişince eski ve yeni süzgeç
//! ~30 ms çapraz geçişle karıştırılır. Kapalıyken ses hiç işlenmez (bit bit aynı).

use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Sample;

/// Seviye farkının (dB) ve kesim frekansının (Hz) sınırları.
pub const MIN_LEVEL_DB: f64 = 1.0;
pub const MAX_LEVEL_DB: f64 = 15.0;
pub const MIN_CUTOFF_HZ: f64 = 300.0;
pub const MAX_CUTOFF_HZ: f64 = 1200.0;
/// Ayar değişince eski ve yeni süzgecin karıştırılma süresi.
const CROSSFADE_SECONDS: f64 = 0.03;

/// Hazır ayarlar: (ad, kesim Hz, seviye farkı dB).
pub const PRESETS: [(&str, f64, f64); 4] = [
    // Jan Meier'in devresi (bs2b "jmeier").
    ("Hafif", 650.0, 9.5),
    // Chu Moy'un devresi (bs2b "cmoy").
    ("Orta", 700.0, 6.0),
    // bs2b'nin varsayılanı.
    ("Güçlü", 700.0, 4.5),
    // Küresel baş modeli, ±30° hoparlör: grup gecikmesi ~0,28 ms; alçak frekansta karşı kulağa
    // ses neredeyse kısılmadan gider (gerçek odada da öyle), 1 dB pay bırakılır.
    ("Hoparlör gibi", 570.0, 1.0),
];

/// Kalıcı crossfeed ayarı.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CrossfeedSettings {
    pub enabled: bool,
    /// Kesim frekansı (Hz): karşı kulağa giden sesin süzülme frekansı; zaman farkı 1/(2π·fc).
    pub cutoff_hz: f64,
    /// Alçak frekansta karşı kulağın aynı kulağa göre ne kadar kısık olduğu (dB).
    pub level_db: f64,
}

impl Default for CrossfeedSettings {
    fn default() -> Self {
        let (_, cutoff_hz, level_db) = PRESETS[1];
        Self {
            enabled: false,
            cutoff_hz,
            level_db,
        }
    }
}

impl CrossfeedSettings {
    pub fn sanitized(self) -> Self {
        let finite = |x: f64, fallback: f64| if x.is_finite() { x } else { fallback };
        let default = Self::default();
        Self {
            enabled: self.enabled,
            cutoff_hz: finite(self.cutoff_hz, default.cutoff_hz)
                .clamp(MIN_CUTOFF_HZ, MAX_CUTOFF_HZ),
            level_db: finite(self.level_db, default.level_db).clamp(MIN_LEVEL_DB, MAX_LEVEL_DB),
        }
    }

    /// Karşı kulağın alçak frekanstaki payı r0 (doğrusal, 0–1).
    pub fn ratio(&self) -> f64 {
        10f64.powf(-self.level_db / 20.0)
    }
}

/// Arayüze giden durum: ayar, hazır ayar adları ve çizim için eğriler.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CrossfeedState {
    pub settings: CrossfeedSettings,
    /// Hazır ayarlar: ad, kesim (Hz), seviye farkı (dB).
    pub presets: Vec<CrossfeedPreset>,
    /// Kulaklar arası zaman farkı (µs, alçak frekansta).
    pub delay_us: f64,
    /// Eğri frekansları (Hz) ve tamamen sola kaydırılmış seste sol/sağ kulağa giden (dB).
    pub curve_hz: Vec<f64>,
    pub near_db: Vec<f64>,
    pub far_db: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CrossfeedPreset {
    pub name: String,
    pub cutoff_hz: f64,
    pub level_db: f64,
}

/// Eğrilerin hesaplandığı örnekleme hızı.
const DISPLAY_RATE: f64 = 96_000.0;

impl CrossfeedState {
    pub fn new(settings: CrossfeedSettings) -> Self {
        let settings = settings.sanitized();
        let side = SideFilter::design(&settings, DISPLAY_RATE);
        let curve_hz: Vec<f64> = (0..=120)
            .map(|i| 20.0 * 1000f64.powf(f64::from(i) / 120.0))
            .collect();
        let (near_db, far_db) = curve_hz
            .iter()
            .map(|&hz| {
                let g = side.response(2.0 * PI * hz / DISPLAY_RATE);
                // Sola kaydırılmış ses: L' = (1 + G)/2, R' = (1 − G)/2.
                let db = |c: (f64, f64)| 10.0 * (c.0 * c.0 + c.1 * c.1).max(1e-300).log10();
                (
                    db(((1.0 + g.0) / 2.0, g.1 / 2.0)),
                    db(((1.0 - g.0) / 2.0, -g.1 / 2.0)),
                )
            })
            .unzip();
        Self {
            settings,
            presets: PRESETS
                .iter()
                .map(|&(name, cutoff_hz, level_db)| CrossfeedPreset {
                    name: name.to_owned(),
                    cutoff_hz,
                    level_db,
                })
                .collect(),
            delay_us: 1e6 / (2.0 * PI * settings.cutoff_hz),
            curve_hz,
            near_db,
            far_db,
        }
    }
}

/// Yan bileşenin süzgeci: G(z) = (b0 + b1·z⁻¹) / (1 + a1·z⁻¹).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SideFilter {
    b0: f64,
    b1: f64,
    a1: f64,
}

impl SideFilter {
    const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        a1: 0.0,
    };

    /// G(s) = (s + z)/(s + p), z = ωc(1 − r0), p = ωc(1 + r0); çift doğrusal dönüşüm, z ile p'nin
    /// geometrik ortasında ön bükme (orada analog ile tam aynı).
    fn design(settings: &CrossfeedSettings, sample_rate: f64) -> Self {
        let r0 = settings.ratio().clamp(0.0, 0.999);
        let wc = 2.0 * PI * settings.cutoff_hz;
        let (zero, pole) = (wc * (1.0 - r0), wc * (1.0 + r0));
        let wm = (zero * pole).sqrt();
        let t = wm / (2.0 * sample_rate);
        if !(t > 0.0 && t < PI / 2.0 * 0.99) {
            return Self::IDENTITY;
        }
        let k = wm / t.tan();
        let norm = k + pole;
        Self {
            b0: (k + zero) / norm,
            b1: (zero - k) / norm,
            a1: (pole - k) / norm,
        }
    }

    /// `w` (radyan/örnek) açısal frekansındaki karmaşık yanıt (gerçek, sanal).
    fn response(&self, w: f64) -> (f64, f64) {
        let (s, c) = w.sin_cos();
        // e^{−jw} = c − j·s
        let (nr, ni) = (self.b0 + self.b1 * c, -self.b1 * s);
        let (dr, di) = (1.0 + self.a1 * c, -self.a1 * s);
        let d = dr * dr + di * di;
        ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d)
    }

    #[inline]
    fn process(&self, state: &mut f64, x: Sample) -> Sample {
        // Devrik doğrudan biçim II (birinci dereceden).
        let y = self.b0 * x + *state;
        *state = self.b1 * x - self.a1 * y;
        y
    }
}

/// Arayüzden ses iş parçacığına ayar taşıyan kilitsiz kanal.
#[derive(Debug)]
pub struct CrossfeedControl {
    version: AtomicU64,
    enabled: AtomicBool,
    cutoff_hz: AtomicU64,
    level_db: AtomicU64,
}

impl Default for CrossfeedControl {
    fn default() -> Self {
        let control = Self {
            version: AtomicU64::new(0),
            enabled: AtomicBool::new(false),
            cutoff_hz: AtomicU64::new(0),
            level_db: AtomicU64::new(0),
        };
        control.set(CrossfeedSettings::default());
        control
    }
}

impl CrossfeedControl {
    pub fn set(&self, settings: CrossfeedSettings) {
        let settings = settings.sanitized();
        self.cutoff_hz
            .store(settings.cutoff_hz.to_bits(), Ordering::Relaxed);
        self.level_db
            .store(settings.level_db.to_bits(), Ordering::Relaxed);
        self.enabled.store(settings.enabled, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    fn settings(&self) -> CrossfeedSettings {
        CrossfeedSettings {
            enabled: self.enabled.load(Ordering::Relaxed),
            cutoff_hz: f64::from_bits(self.cutoff_hz.load(Ordering::Relaxed)),
            level_db: f64::from_bits(self.level_db.load(Ordering::Relaxed)),
        }
    }
}

/// Bir süzgeç takımı: kapalıysa `None`.
#[derive(Debug, Clone, Copy)]
struct Stage {
    filter: Option<SideFilter>,
    state: f64,
}

impl Stage {
    fn new(settings: &CrossfeedSettings, sample_rate: f64) -> Self {
        Self {
            filter: settings
                .enabled
                .then(|| SideFilter::design(settings, sample_rate)),
            state: 0.0,
        }
    }

    /// Bir karenin (L, R) çıkışı.
    #[inline]
    fn process(&mut self, left: Sample, right: Sample) -> (Sample, Sample) {
        let Some(filter) = self.filter else {
            return (left, right);
        };
        let mid = (left + right) * 0.5;
        let side = filter.process(&mut self.state, (left - right) * 0.5);
        (mid + side, mid - side)
    }
}

/// Crossfeed'i uygulayan gerçek zamanlı işlemci (yalnızca stereo).
pub struct CrossfeedProcessor {
    control: Arc<CrossfeedControl>,
    stereo: bool,
    sample_rate: f64,
    seen_version: u64,
    current: Stage,
    next: Stage,
    fade_left: usize,
    fade_frames: usize,
}

impl CrossfeedProcessor {
    pub fn new(control: Arc<CrossfeedControl>, channels: usize, sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let seen_version = control.version();
        let current = Stage::new(&control.settings(), rate);
        Self {
            control,
            stereo: channels == 2,
            sample_rate: rate,
            seen_version,
            current,
            next: current,
            fade_left: 0,
            fade_frames: ((CROSSFADE_SECONDS * rate) as usize).max(1),
        }
    }

    /// Ses şu an değişmeden mi geçiyor?
    pub fn is_bypass(&self) -> bool {
        !self.stereo || (self.fade_left == 0 && self.current.filter.is_none())
    }

    /// Her doldurma turunun başında: yeni ayar varsa çapraz geçişi başlatır.
    pub fn begin_block(&mut self) {
        let version = self.control.version();
        if version != self.seen_version && self.fade_left == 0 {
            self.seen_version = version;
            self.next = Stage::new(&self.control.settings(), self.sample_rate);
            // Yeni süzgeç, eskisinin durumundan başlar (aynı yan sinyali süzüyor): geçiş
            // daha da yumuşak olur.
            self.next.state = self.current.state;
            self.fade_left = self.fade_frames;
        }
        for stage in [&mut self.current, &mut self.next] {
            if stage.state.abs() < 1e-200 {
                stage.state = 0.0;
            }
        }
    }

    /// Bir karenin iki kanalını yerinde işler.
    #[inline]
    pub fn process_frame(&mut self, frame: &mut [Sample]) {
        if !self.stereo {
            return;
        }
        let [left, right] = frame else {
            return;
        };
        if self.fade_left == 0 {
            (*left, *right) = self.current.process(*left, *right);
            return;
        }
        let t = 1.0 - self.fade_left as f64 / self.fade_frames as f64;
        let (ol, or) = self.current.process(*left, *right);
        let (nl, nr) = self.next.process(*left, *right);
        *left = ol + (nl - ol) * t;
        *right = or + (nr - or) * t;
        self.fade_left -= 1;
        if self.fade_left == 0 {
            std::mem::swap(&mut self.current, &mut self.next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(cutoff_hz: f64, level_db: f64) -> CrossfeedSettings {
        CrossfeedSettings {
            enabled: true,
            cutoff_hz,
            level_db,
        }
    }

    fn processor(settings: CrossfeedSettings, rate: u32) -> CrossfeedProcessor {
        let control = Arc::new(CrossfeedControl::default());
        control.set(settings);
        CrossfeedProcessor::new(control, 2, rate)
    }

    /// Ortadaki ses (L = R) bit bit aynı çalar: vokal ve bas hiç renklenmez.
    #[test]
    fn ortadaki_ses_bit_bit_ayni() {
        for (name, cutoff, level) in PRESETS {
            let mut p = processor(on(cutoff, level), 44_100);
            for i in 0..20_000 {
                let x = 0.9 * (f64::from(i) * 0.37).sin() * (f64::from(i) * 0.0011).cos();
                let mut frame = [x, x];
                p.process_frame(&mut frame);
                assert_eq!(frame, [x, x], "{name}, örnek {i}");
            }
        }
    }

    #[test]
    fn kapaliyken_ses_bit_bit_ayni() {
        let mut p = processor(CrossfeedSettings::default(), 48_000);
        assert!(p.is_bypass());
        let mut frame = [0.123, -0.456];
        p.process_frame(&mut frame);
        assert_eq!(frame, [0.123, -0.456]);
        // Mono ya da çok kanallı akışa dokunulmaz.
        let control = Arc::new(CrossfeedControl::default());
        control.set(on(700.0, 6.0));
        let mut mono = CrossfeedProcessor::new(control, 1, 48_000);
        let mut frame = [0.5];
        mono.process_frame(&mut frame);
        assert_eq!(frame, [0.5]);
    }

    /// Yan süzgeç hiçbir frekansta yükseltmez (|G| ≤ 1), DC'de (1 − r0)/(1 + r0), Nyquist'te 1.
    #[test]
    fn yan_suzgec_hicbir_frekansi_yukseltmez() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for cutoff in [MIN_CUTOFF_HZ, 650.0, MAX_CUTOFF_HZ] {
                for level in [MIN_LEVEL_DB, 4.5, 9.5, MAX_LEVEL_DB] {
                    let settings = on(cutoff, level);
                    let g = SideFilter::design(&settings, rate);
                    for i in 0..=2000 {
                        let w = PI * f64::from(i) / 2000.0;
                        let (re, im) = g.response(w);
                        assert!(
                            re * re + im * im <= 1.0 + 1e-12,
                            "{rate} {cutoff} {level} {w}"
                        );
                    }
                    let r0 = settings.ratio();
                    let (dc, _) = g.response(0.0);
                    assert!((dc - (1.0 - r0) / (1.0 + r0)).abs() < 1e-12);
                    let (ny, _) = g.response(PI);
                    assert!((ny - 1.0).abs() < 1e-12);
                }
            }
        }
    }

    /// Sola kaydırılmış seste kulaklar arası oran, hedeflenen ρ = r0/(1 + jf/fc): alçak
    /// frekansta seviye farkı `level_db`, zaman farkı 1/(2π·fc).
    #[test]
    fn kulaklar_arasi_seviye_ve_zaman_farki_hedefte() {
        let rate = 48_000.0;
        let settings = on(700.0, 6.0);
        let g = SideFilter::design(&settings, rate);
        let r0 = settings.ratio();
        // Kulaklar arası zaman farkının anlamlı olduğu bant (< 2 kHz); üstünde çift doğrusal
        // dönüşümün frekans bükülmesi birkaç yüzde (karşı kulağın payı orada zaten −20 dB altında).
        for hz in [20.0, 100.0, 300.0, 700.0, 1500.0, 2000.0] {
            let (re, im) = g.response(2.0 * PI * hz / rate);
            // ρ = (1 − G)/(1 + G)
            let (nr, ni) = (1.0 - re, -im);
            let (dr, di) = (1.0 + re, im);
            let d = dr * dr + di * di;
            let (rr, ri) = ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d);
            // Hedef (ön bükme farkı < 2 kHz'te ihmal edilebilir).
            let x = hz / 700.0;
            let (tr, ti) = (r0 / (1.0 + x * x), -r0 * x / (1.0 + x * x));
            let err = ((rr - tr).powi(2) + (ri - ti).powi(2)).sqrt() / (tr * tr + ti * ti).sqrt();
            assert!(err < 0.01, "{hz} Hz: göreli hata {err}");
        }
        // Alçak frekanstaki zaman farkı: −faz/ω.
        let hz = 30.0;
        let w = 2.0 * PI * hz / rate;
        let (re, im) = g.response(w);
        let (nr, ni) = (1.0 - re, -im);
        let (dr, di) = (1.0 + re, im);
        let phase = (ni * dr - nr * di).atan2(nr * dr + ni * di);
        let delay = -phase / (2.0 * PI * hz);
        let expected = 1.0 / (2.0 * PI * 700.0);
        assert!(
            (delay - expected).abs() / expected < 0.01,
            "{delay} != {expected}"
        );
    }

    /// Tamamen sola kaydırılmış bir sinüs: sağ kulağa da gider (alçak frekansta ~6 dB kısık),
    /// sol kulak ve sağ kulak toplamı hiçbir zaman girişten yüksek değildir.
    #[test]
    fn sola_kaydirilmis_ses_karsi_kulaga_da_gider() {
        let rate = 48_000u32;
        let mut p = processor(on(700.0, 6.0), rate);
        let w = 2.0 * PI * 100.0 / f64::from(rate);
        let (mut peak_l, mut peak_r) = (0.0f64, 0.0f64);
        for i in 0..rate {
            let mut frame = [(w * f64::from(i)).sin(), 0.0];
            p.process_frame(&mut frame);
            if i > rate / 2 {
                peak_l = peak_l.max(frame[0].abs());
                peak_r = peak_r.max(frame[1].abs());
            }
        }
        let ild = 20.0 * (peak_l / peak_r).log10();
        assert!((ild - 6.0).abs() < 0.3, "kulaklar arası fark {ild:.2} dB");
        assert!(peak_l <= 1.0 && peak_r <= 1.0);
    }

    #[test]
    fn ayar_degisirken_ani_sicrama_olmaz() {
        let rate = 48_000u32;
        let control = Arc::new(CrossfeedControl::default());
        let mut p = CrossfeedProcessor::new(Arc::clone(&control), 2, rate);
        let w = 2.0 * PI * 150.0 / f64::from(rate);
        let mut previous = [0.0, 0.0];
        let mut biggest = 0.0f64;
        for i in 0..rate as usize {
            if i == rate as usize / 3 {
                control.set(on(700.0, 4.5));
            }
            if i == 2 * rate as usize / 3 {
                control.set(CrossfeedSettings::default());
            }
            if i % 256 == 0 {
                p.begin_block();
            }
            let x = 0.5 * (w * i as f64).sin();
            let mut frame = [x, -0.3 * x];
            p.process_frame(&mut frame);
            for c in 0..2 {
                biggest = biggest.max((frame[c] - previous[c]).abs());
            }
            previous = frame;
        }
        // 150 Hz, 0,5 genlikli sinüsün örnekler arası en büyük adımı ~0,0098.
        assert!(biggest < 0.015, "{biggest}");
        assert!(p.is_bypass(), "kapatınca geçiş bitti");
    }

    #[test]
    fn durum_egrileri_ve_ayar_sinirlari() {
        let state = CrossfeedState::new(on(700.0, 6.0));
        assert_eq!(state.curve_hz.len(), state.near_db.len());
        assert_eq!(state.curve_hz.len(), state.far_db.len());
        // Alçak frekansta yakın ve uzak kulak arasında 6 dB; yüksek frekansta uzak kulak sessiz.
        assert!(((state.near_db[0] - state.far_db[0]) - 6.0).abs() < 0.2);
        assert!(state.far_db.last().copied().unwrap_or(0.0) < -25.0);
        // Yakın kulak hiçbir frekansta 0 dB'yi geçmez.
        assert!(state.near_db.iter().all(|&db| db <= 1e-9));
        assert!((state.delay_us - 227.4).abs() < 0.5);
        let wild = CrossfeedSettings {
            enabled: true,
            cutoff_hz: 1e9,
            level_db: f64::NAN,
        }
        .sanitized();
        assert_eq!(wild.cutoff_hz, MAX_CUTOFF_HZ);
        assert_eq!(wild.level_db, CrossfeedSettings::default().level_db);
        let json = serde_json::to_value(&state).unwrap();
        assert!(json["settings"]["cutoffHz"].is_number());
        assert_eq!(json["presets"][3]["name"], "Hoparlör gibi");
    }
}
