//! Bas motoru: "Bas" düğmesi, "Derinlik" (alt oktav) ve küçük hoparlörler için
//! psikoakustik bas.
//!
//! - **Bas düğmesi** (0–18 dB; 12'nin üstü kulüp düzeyi): 100 Hz alçak raf süzgeci. Grafik
//!   ekolayzerden bağımsız, tek dokunuşla bas.
//! - **Derinlik** (alt oktav, %0–100): bas notalarının bir oktav altı üretilip eklenir;
//!   kulüpteki gibi göğüste hissedilen gümbürtü, kayıtta derin bas olmasa da. Bas bandının
//!   (40–120 Hz) her tam dalgasında işaret değiştiren bir kare dalga, bandın zarfıyla
//!   çarpılır ve dördüncü dereceden 60 Hz süzgeçle yumuşatılır (klasik oktav bölücü).
//! - **Vuruş** (%0–100; [`super::punch`]): davul vuruşlarının ilk anı 8 dB'ye kadar
//!   güçlenir, sürekli bas aynen kalır. Bas şişmeden göğse çarpan vuruş.
//! - **Küçük hoparlör bası**: dizüstü ve küçük hoparlörler ~100 Hz'in altını çalamaz;
//!   oradaki bası yükseltmek yalnızca bozulma yaratır. Bunun yerine basın 2., 3. ve 4.
//!   harmonikleri üretilip eklenir: beyin, duyulmayan temel notayı harmoniklerden
//!   tamamlar ("eksik temel" etkisi). Hoparlörün çalamadığı alt bas (ve alt oktav) süzülür.
//!
//! Harmonikler Chebyshev polinomlarıyla üretilir: genliği 1'e getirilmiş bir sinüste
//! `T_n(cos θ) = cos(nθ)`, yani n. harmonik tam olarak ve istenen oranda çıkar. Genlik
//! zarfla geri verildiği için harmoniklerin yüksekliği basın yüksekliğini izler.
//!
//! **Taşma koruması** ([`super::normalize`]) bası yükseltince sesi yalnızca gerçekten
//! taşacağı kadar kısar. Her şarkının bas tepeleri analizde ölçülür ([`BassPeakMeter`]):
//! rafın her kazancında şarkının tepesi ne kadar yükseliyor. Çoğu şarkıda tepe bastan
//! değil vokal ve davuldan gelir; bas +12 dB'de bile tepe birkaç dB yükselir. Ölçüm yoksa
//! en kötü durum (rafın tamamı) varsayılır. Vuruşun rafı bas düğmesininkiyle aynı köşede:
//! ikisi tek raf gibi (kazançlar toplanarak) hesaplanır; vuruşun en yüksek anı ayrılır.
//!
//! Ayarlar ekolayzerin kilitsiz kanalından ([`EqControl`]) okunur; değişiklikler ~40 ms'de
//! yumuşakça uygulanır. Hepsi kapalıyken ses hiç işlenmez. Bellek yalnızca kurulurken
//! ayrılır.

use std::f64::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::biquad::{self, Biquad, State};
use super::eq::EqControl;
use super::punch::{Punch, PUNCH_MAX_DB};
use super::Sample;

/// Bas düğmesinin üst sınırı (dB).
pub const MAX_BASS_DB: f64 = 18.0;
/// Bu düzeyin üstü "kulüp bölgesi" (arayüzde ayrı renk).
pub const CLUB_BASS_DB: f64 = 12.0;
/// Raf süzgecinin köşe frekansı (Hz): davulun ve bas gitarın gövdesi bunun altında.
const SHELF_HZ: f64 = 100.0;
/// Harmonikleri üretilecek bas bandının üst sınırı (Hz).
const SPLIT_HZ: f64 = 120.0;
/// Küçük hoparlörde süzülen alt bas (Hz; dördüncü derece, iki Butterworth art arda):
/// hoparlörün çalamadığı enerji gönderilmez, bozulma azalır, harmoniklere yer kalır.
const SUB_CUT_HZ: f64 = 80.0;
/// Eklenen harmoniklerin bandı (Hz).
const HARMONIC_LOW_HZ: f64 = 90.0;
const HARMONIC_HIGH_HZ: f64 = 600.0;
/// 2., 3. ve 4. harmoniklerin temel notaya göre oranları.
const WEIGHTS: [f64; 3] = [0.6, 0.35, 0.15];
/// Harmonik miktarı: küçük hoparlör açıkken bas 0'da bile süzülen alt bası karşılayan
/// taban, bas düğmesiyle (12 dB'de) tavana kadar artar.
const HARMONIC_BASE: f64 = 0.5;
const HARMONIC_MAX: f64 = 1.5;
const HARMONIC_FULL_DB: f64 = 12.0;
/// Bas zarfının iniş zaman sabiti (s). Zarf anında yükselir: bas birden başlasa da
/// harmonik üretici doyuma girmez (kare dalga, tık olmaz).
const ENVELOPE_RELEASE_SECONDS: f64 = 0.15;
/// Alt oktavın kaynak bandı (Hz) ve sonucu yumuşatan süzgeçler.
const OCTAVE_LOW_HZ: f64 = 40.0;
const OCTAVE_HIGH_HZ: f64 = 120.0;
const OCTAVE_SMOOTH_HZ: f64 = 60.0;
const OCTAVE_DC_HZ: f64 = 18.0;
/// Alt oktavın genliği (derinlik %100'de, kaynak bas notasına göre; kare dalganın temel
/// bileşeni 4/π × zarf).
const OCTAVE_GAIN: f64 = 0.8;
/// Alt oktavın, kaynak bandın tepesine göre en büyük genliği (ölçülen ~1,0; pay ile).
const OCTAVE_PEAK: f64 = 1.1;
/// Oktav bölücünün kararsız titremesini önleyen eşik (zarfın oranı).
const OCTAVE_HYSTERESIS: f64 = 0.25;
/// Bas bandı sesin geri kalanından bu kadar (−40 dB) kısıksa alt oktav yumuşakça susar:
/// ortadaki sesten sızan çok küçük sinyal bölücüyü oynatmasın (iz kalmasın).
const OCTAVE_GATE: f64 = 0.01;
/// Ayar değişikliklerinin yumuşatılması: her 2 ms'de bir, ~40 ms'lik zaman sabitiyle.
const UPDATE_SECONDS: f64 = 0.002;
const SMOOTH_SECONDS: f64 = 0.040;
/// Hedefe bu kadar yaklaşınca doğrudan hedefe oturulur (dB).
const SNAP_DB: f64 = 0.01;
/// Küçük hoparlörde harmonikler için taşma korumasına eklenen pay (dB). Ölçülen: süzgeçlerin
/// en büyük artışının üstüne en fazla ~2,6 dB (`yukseltme_payi_tepeleri_karsilar` testi).
const HARMONIC_ALLOWANCE_DB: f64 = 3.0;
/// Ölçülen bas tepelerine eklenen güvenlik payı (dB): çalma hızı analizinkinden farklı
/// olabilir, ara değerler doğrusal tahmin edilir.
const MEASURED_MARGIN_DB: f64 = 0.5;
/// Bas tepelerinin ölçüldüğü raf kazançları (dB): bas düğmesi ve vuruş birlikte (18 + 8).
pub const PEAK_STEPS_DB: [f64; 9] = [3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0, 24.0, 27.0];
const STEPS: usize = PEAK_STEPS_DB.len();
// Bas düğmesi ve vuruş birlikte en yüksekteyken de tepe ölçümü yetişir (derlemede denetlenir).
const _: () = assert!(PEAK_STEPS_DB[STEPS - 1] >= MAX_BASS_DB + PUNCH_MAX_DB);

/// Düğmenin ayarından harmonik miktarı.
fn harmonic_amount(bass_db: f64) -> f64 {
    HARMONIC_BASE + (HARMONIC_MAX - HARMONIC_BASE) * (bass_db / HARMONIC_FULL_DB).clamp(0.0, 1.0)
}

fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

fn linear_to_db(linear: f64) -> f64 {
    20.0 * linear.max(1e-12).log10()
}

/// Bas motorunun doğrusal kısmının (raf ve alt bas süzgeci) `hz`'deki kazancı (dB).
/// Harmonikler ve alt oktav doğrusal olmadığı için eğride gösterilmez.
pub fn response_db(bass_db: f64, small_speaker: bool, hz: f64, sample_rate: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * hz / sample_rate;
    let shelf = Biquad::low_shelf(SHELF_HZ, bass_db, FRAC_1_SQRT_2, sample_rate).response_db(w);
    let cut = if small_speaker {
        2.0 * Biquad::highpass(SUB_CUT_HZ, FRAC_1_SQRT_2, sample_rate).response_db(w)
    } else {
        0.0
    };
    shelf + cut
}

/// Küçük hoparlör kipinin en büyük yükseltmesi (dB). Alt bas süzüldüğü için rafın tamamı
/// tepeye eklenmez: süzgeçlerin en büyük artışı (30–300 Hz taranır) ve harmonikler için
/// pay. Fazla pay sesi gereksiz kısardı; dizüstünde ses yüksekliği değerlidir.
fn small_speaker_boost_db(bass_db: f64) -> f64 {
    let peak = (0..=48)
        .map(|i| 30.0 * 10f64.powf(f64::from(i) / 48.0))
        .map(|hz| response_db(bass_db, true, hz, REFERENCE_RATE))
        .fold(0.0, f64::max);
    peak + HARMONIC_ALLOWANCE_DB
}

/// Yükseltme hesabında kullanılan örnekleme hızı (alt frekanslarda yanıt hıza bağlı değil).
const REFERENCE_RATE: f64 = 48_000.0;

/// Bir şarkının bas tepeleri (analizde ölçülür, önbellekte saklanır).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BassPeaks {
    /// [`PEAK_STEPS_DB`] kazançlarındaki raf, şarkının tepesini kaç dB yükseltiyor.
    pub shelf_rise_db: [f64; STEPS],
    /// Alt oktavın kaynak bandının (40–120 Hz) tepesi, şarkının tepesine göre (dB, ≤ 0).
    pub octave_band_db: f64,
}

impl BassPeaks {
    /// `bass_db` kazançlı rafın şarkının tepesini yükseltmesi (dB; ara değerler doğrusal,
    /// güvenlik payıyla).
    pub fn shelf_rise_db(&self, bass_db: f64) -> f64 {
        if bass_db <= 0.0 {
            return 0.0;
        }
        let mut previous = (0.0, 0.0);
        for (&step, &rise) in PEAK_STEPS_DB.iter().zip(&self.shelf_rise_db) {
            if bass_db <= step {
                let t = (bass_db - previous.0) / (step - previous.0);
                return previous.1 + (rise - previous.1) * t + MEASURED_MARGIN_DB;
            }
            previous = (step, rise);
        }
        previous.1 + MEASURED_MARGIN_DB
    }
}

/// Analizde bas tepelerini ölçer: her kanal [`PEAK_STEPS_DB`] kazançlarındaki raflardan
/// geçirilir, tepeleri tutulur.
pub struct BassPeakMeter {
    shelves: [Biquad; STEPS],
    states: Vec<[State; STEPS]>,
    peaks: [f64; STEPS],
    raw_peak: f64,
    band: [Biquad; 2],
    band_state: [State; 3],
    band_peak: f64,
}

impl BassPeakMeter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let rate = f64::from(sample_rate.max(1));
        Self {
            shelves: PEAK_STEPS_DB.map(|db| Biquad::low_shelf(SHELF_HZ, db, FRAC_1_SQRT_2, rate)),
            states: vec![[[0.0; 2]; STEPS]; channels.max(1)],
            peaks: [0.0; STEPS],
            raw_peak: 0.0,
            band: [
                Biquad::highpass(OCTAVE_LOW_HZ, FRAC_1_SQRT_2, rate),
                Biquad::lowpass(OCTAVE_HIGH_HZ, FRAC_1_SQRT_2, rate),
            ],
            band_state: [[0.0; 2]; 3],
            band_peak: 0.0,
        }
    }

    pub fn add(&mut self, frame: &[Sample]) {
        let mut mono = 0.0;
        for (x, states) in frame.iter().zip(self.states.iter_mut()) {
            let x = if x.is_finite() { *x } else { 0.0 };
            mono += x;
            self.raw_peak = self.raw_peak.max(x.abs());
            for ((shelf, state), peak) in self.shelves.iter().zip(states).zip(&mut self.peaks) {
                *peak = peak.max(shelf.process(state, x).abs());
            }
        }
        let mono = mono / frame.len().max(1) as f64;
        let b = self.band[0].process(&mut self.band_state[0], mono);
        let b = self.band[1].process(&mut self.band_state[1], b);
        let b = self.band[1].process(&mut self.band_state[2], b);
        self.band_peak = self.band_peak.max(b.abs());
    }

    /// Ölçüm (sessiz şarkıda `None`).
    pub fn finish(&self) -> Option<BassPeaks> {
        (self.raw_peak > 1e-9).then(|| BassPeaks {
            shelf_rise_db: self.peaks.map(|p| linear_to_db(p / self.raw_peak).max(0.0)),
            octave_band_db: linear_to_db(self.band_peak / self.raw_peak).min(0.0),
        })
    }
}

/// Bas motorunun taşma korumasına bildirdiği yükseltme. Şarkının bas tepeleri ölçüldüyse
/// gerçek artış ([`BassBoost::rise_db`]), ölçülmediyse en kötü durum kullanılır.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BassBoost {
    /// Raf kazancı (dB).
    pub shelf_db: f64,
    /// Vuruşun en büyük raf kazancı (dB; vuruşun en yüksek anı, rafa eklenir).
    pub punch_db: f64,
    /// Derinlik (0..1).
    pub depth: f64,
    /// Ölçümden bağımsız en küçük yükseltme (dB): küçük hoparlör kipi ve geçişleri.
    pub floor_db: f64,
}

impl BassBoost {
    /// Şarkının tepesinin en fazla kaç dB yükseleceği.
    pub fn rise_db(&self, peaks: Option<&BassPeaks>) -> f64 {
        let total = self.shelf_db + self.punch_db;
        let shelf = peaks.map_or(total.max(0.0), |p| p.shelf_rise_db(total));
        let band = peaks.map_or(1.0, |p| db_to_linear(p.octave_band_db));
        let octave = self.depth.max(0.0) * OCTAVE_PEAK * band;
        linear_to_db(db_to_linear(shelf) + octave).max(self.floor_db)
    }
}

/// Ayarlardan bas motorunun en kötü durum yükseltmesi (dB; arayüzdeki koruma göstergesi ve
/// ölçüm yokken).
pub fn boost_db(bass_db: f64, depth: f64, punch: f64, small_speaker: bool) -> f64 {
    settings_boost(bass_db, depth, punch, small_speaker).rise_db(None)
}

/// Ayarlardan bas motorunun yükseltmesi (ölçüm uygulanmadan). `punch`: vuruş (0..1).
pub fn settings_boost(bass_db: f64, depth: f64, punch: f64, small_speaker: bool) -> BassBoost {
    let punch_db = punch.clamp(0.0, 1.0) * PUNCH_MAX_DB;
    if small_speaker {
        // Alt oktav küçük hoparlörde süzülür; vuruşun rafı da alt bas süzgecinden geçer.
        BassBoost {
            shelf_db: 0.0,
            punch_db: 0.0,
            depth: 0.0,
            floor_db: small_speaker_boost_db(bass_db + punch_db),
        }
    } else {
        BassBoost {
            shelf_db: bass_db,
            punch_db,
            depth,
            floor_db: 0.0,
        }
    }
}

/// Alt oktav üretici ("Derinlik").
struct Octaver {
    band_low: Biquad,
    band_high: Biquad,
    band_state: [State; 3],
    envelope: f64,
    /// Bütün sesin tepe zarfı (eşik için).
    full_envelope: f64,
    release: f64,
    /// Bas dalgası eşiğin altına indi: bir sonraki yükselişte işaret değişir.
    armed: bool,
    sign: f64,
    smooth: Biquad,
    smooth_state: [State; 2],
    dc: Biquad,
    dc_state: State,
}

impl Octaver {
    fn new(rate: f64) -> Self {
        let q = FRAC_1_SQRT_2;
        Self {
            band_low: Biquad::highpass(OCTAVE_LOW_HZ, q, rate),
            band_high: Biquad::lowpass(OCTAVE_HIGH_HZ, q, rate),
            band_state: [[0.0; 2]; 3],
            envelope: 0.0,
            full_envelope: 0.0,
            release: (-1.0 / (ENVELOPE_RELEASE_SECONDS * rate)).exp(),
            armed: false,
            sign: 1.0,
            smooth: Biquad::lowpass(OCTAVE_SMOOTH_HZ, q, rate),
            smooth_state: [[0.0; 2]; 2],
            dc: Biquad::highpass(OCTAVE_DC_HZ, q, rate),
            dc_state: [0.0; 2],
        }
    }

    /// Bir örneğin alt oktavı (derinlik %100'deki genlikte).
    #[inline]
    fn process(&mut self, mono: Sample) -> Sample {
        let b = self.band_low.process(&mut self.band_state[0], mono);
        let b = self.band_high.process(&mut self.band_state[1], b);
        let b = self.band_high.process(&mut self.band_state[2], b);
        self.envelope = b.abs().max(self.envelope * self.release);
        self.full_envelope = mono.abs().max(self.full_envelope * self.release);
        // Her tam dalgada bir kez (yükselen geçişte) işaret değişir: frekansın yarısı.
        let threshold = OCTAVE_HYSTERESIS * self.envelope;
        if b < -threshold {
            self.armed = true;
        } else if self.armed && b > threshold {
            self.armed = false;
            self.sign = -self.sign;
        }
        let presence = (self.envelope / (self.full_envelope * OCTAVE_GATE).max(1e-12)).min(1.0);
        let square = self.sign * self.envelope * presence * OCTAVE_GAIN;
        let y = self.smooth.process(&mut self.smooth_state[0], square);
        let y = self.smooth.process(&mut self.smooth_state[1], y);
        self.dc.process(&mut self.dc_state, y)
    }

    fn flush(&mut self) {
        for state in self
            .band_state
            .iter_mut()
            .chain(self.smooth_state.iter_mut())
            .chain(std::iter::once(&mut self.dc_state))
        {
            biquad::flush(state);
        }
    }
}

/// Bas motorunu uygulayan gerçek zamanlı işlemci.
pub struct BassProcessor {
    control: Arc<EqControl>,
    seen_version: u64,
    sample_rate: f64,
    /// Raf süzgecinin şu anki ve hedef kazancı (dB).
    shelf_db: f64,
    target_db: f64,
    shelf: Biquad,
    shelf_state: Vec<State>,
    /// Küçük hoparlör kipinin karışım oranı (0..1) ve hedefi.
    mix: f64,
    mix_target: f64,
    /// Derinlik (0..1) ve hedefi.
    depth: f64,
    depth_target: f64,
    /// Vuruş (0..1) ve hedefi.
    punch_amount: f64,
    punch_target: f64,
    punch: Punch,
    /// Karışım ve derinlik her örnekte bu kadar ilerler: basamaklı geçiş "fermuar" sesi yapar.
    ramp_step: f64,
    /// Bas bandını ayıran Linkwitz-Riley süzgeci (iki Butterworth art arda).
    split: Biquad,
    split_state: [State; 2],
    /// Bas bandının tepe zarfı ve iniş katsayısı.
    envelope: f64,
    release: f64,
    band_low: Biquad,
    band_high: Biquad,
    band_state: [State; 3],
    sub_cut: Biquad,
    sub_state: Vec<[State; 2]>,
    octaver: Octaver,
    update_frames: usize,
    update_left: usize,
    smooth_coef: f64,
    /// Şu anki yükseltme (taşma korumasına); ayar değişince yeniden hesaplanır.
    boost: BassBoost,
    /// Yükseltme yeniden hesaplanmalı mı?
    boost_dirty: bool,
}

impl BassProcessor {
    pub fn new(control: Arc<EqControl>, channels: usize, sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let channels = channels.max(1);
        let update_frames = ((UPDATE_SECONDS * rate) as usize).max(1);
        let q = FRAC_1_SQRT_2;
        let mut processor = Self {
            seen_version: u64::MAX,
            control,
            sample_rate: rate,
            shelf_db: 0.0,
            target_db: 0.0,
            shelf: Biquad::IDENTITY,
            shelf_state: vec![[0.0; 2]; channels],
            mix: 0.0,
            mix_target: 0.0,
            depth: 0.0,
            depth_target: 0.0,
            punch_amount: 0.0,
            punch_target: 0.0,
            punch: Punch::new(channels, sample_rate),
            ramp_step: 1.0 / (SMOOTH_SECONDS * rate),
            split: Biquad::lowpass(SPLIT_HZ, q, rate),
            split_state: [[0.0; 2]; 2],
            envelope: 0.0,
            release: (-1.0 / (ENVELOPE_RELEASE_SECONDS * rate)).exp(),
            band_low: Biquad::highpass(HARMONIC_LOW_HZ, q, rate),
            band_high: Biquad::lowpass(HARMONIC_HIGH_HZ, q, rate),
            band_state: [[0.0; 2]; 3],
            sub_cut: Biquad::highpass(SUB_CUT_HZ, q, rate),
            sub_state: vec![[[0.0; 2]; 2]; channels],
            octaver: Octaver::new(rate),
            update_frames,
            update_left: 0,
            smooth_coef: 1.0 - (-(update_frames as f64) / (SMOOTH_SECONDS * rate)).exp(),
            boost: BassBoost::default(),
            boost_dirty: true,
        };
        // İlk ayar yumuşatmadan uygulanır (şarkı zaten sessizlikten başlar).
        processor.begin_block();
        processor.shelf_db = processor.target_db;
        processor.mix = processor.mix_target;
        processor.depth = processor.depth_target;
        processor.punch_amount = processor.punch_target;
        processor.shelf = Biquad::low_shelf(SHELF_HZ, processor.shelf_db, q, rate);
        processor.refresh_boost();
        processor
    }

    /// Ses şu an hiç işlenmiyor mu?
    pub fn is_bypass(&self) -> bool {
        self.shelf_db == 0.0
            && self.target_db == 0.0
            && self.mix == 0.0
            && self.mix_target == 0.0
            && self.depth == 0.0
            && self.depth_target == 0.0
            && self.punch_amount == 0.0
            && self.punch_target == 0.0
    }

    /// Şu anki yükseltme (taşma korumasına). Gerçek zamanlı yolda her turda okunur; hesap
    /// yalnızca ayar değişirken yapılır.
    pub fn boost(&self) -> BassBoost {
        self.boost
    }

    /// Yükseltmeyi yeniden hesaplar: geçiş sürerken hedefle büyüğü; küçük hoparlöre geçiş
    /// bitmeden (iki yol karışırken) iki yolun büyüğü.
    fn refresh_boost(&mut self) {
        let bass_db = self.shelf_db.max(self.target_db);
        let depth = self.depth.max(self.depth_target);
        let punch = self.punch_amount.max(self.punch_target);
        let full = settings_boost(bass_db, depth, punch, false);
        self.boost = if self.mix == 0.0 && self.mix_target == 0.0 {
            full
        } else if self.mix == 1.0 && self.mix_target == 1.0 {
            settings_boost(bass_db, depth, punch, true)
        } else {
            BassBoost {
                floor_db: small_speaker_boost_db(bass_db + full.punch_db),
                ..full
            }
        };
        self.boost_dirty = false;
    }

    /// Her doldurma turunun başında: ayar değiştiyse hedefleri günceller.
    pub fn begin_block(&mut self) {
        let version = self.control.version();
        if version != self.seen_version {
            self.seen_version = version;
            let settings = self.control.settings();
            let on = settings.enabled;
            self.target_db = if on { settings.bass_db } else { 0.0 };
            self.mix_target = if on && settings.small_speaker {
                1.0
            } else {
                0.0
            };
            self.depth_target = if on { settings.bass_depth } else { 0.0 };
            let punch_target = if on { settings.bass_punch } else { 0.0 };
            if punch_target > 0.0 && self.punch_amount == 0.0 && self.punch_target == 0.0 {
                // Kapalıyken açıldı: algılayıcı önce şarkının bas düzeyini öğrensin.
                self.punch.restart();
            }
            self.punch_target = punch_target;
            // Yeni hedef hemen korumaya girsin (yükseltme gelmeden ses kısılmış olsun).
            self.boost_dirty = true;
        }
        if self.boost_dirty {
            self.refresh_boost();
        }
        for state in self
            .shelf_state
            .iter_mut()
            .chain(self.sub_state.iter_mut().flatten())
            .chain(self.split_state.iter_mut())
            .chain(self.band_state.iter_mut())
        {
            biquad::flush(state);
        }
        self.octaver.flush();
        self.punch.flush();
    }

    /// Ayarları hedefe bir adım yaklaştırır (her `update_frames` karede bir).
    fn step(&mut self) {
        if self.shelf_db != self.target_db {
            let next = self.shelf_db + (self.target_db - self.shelf_db) * self.smooth_coef;
            self.shelf_db = if (next - self.target_db).abs() < SNAP_DB {
                self.target_db
            } else {
                next
            };
            self.shelf =
                Biquad::low_shelf(SHELF_HZ, self.shelf_db, FRAC_1_SQRT_2, self.sample_rate);
            self.boost_dirty = true;
        }
    }

    /// `value`'yu `target`'a bir örneklik adımla yaklaştırır; vardıysa `true`.
    #[inline]
    fn ramp(value: &mut f64, target: f64, step: f64) -> bool {
        let delta = target - *value;
        if delta.abs() <= step {
            *value = target;
            true
        } else {
            *value += step.copysign(delta);
            false
        }
    }

    /// Bas bandından harmonikler (bütün kanallara eklenir).
    #[inline]
    fn harmonics(&mut self, mono: Sample) -> Sample {
        let low = self.split.process(&mut self.split_state[0], mono);
        let low = self.split.process(&mut self.split_state[1], low);
        self.envelope = low.abs().max(self.envelope * self.release);
        let amplitude = self.envelope;
        // Zarf her zaman örneğin üstünde: |u| ≤ 1 (çok kısıkta 0'a bölünmesin).
        let u = (low / amplitude.max(1e-9)).clamp(-1.0, 1.0);
        let u2 = u * u;
        let t2 = 2.0 * u2 - 1.0;
        let t3 = u * (4.0 * u2 - 3.0);
        let t4 = 8.0 * u2 * (u2 - 1.0) + 1.0;
        let shaped = WEIGHTS[0] * t2 + WEIGHTS[1] * t3 + WEIGHTS[2] * t4;
        let h = shaped * amplitude * harmonic_amount(self.shelf_db);
        // Bandın dışını (doğru akım, alt bas, yüksek dereceler) at.
        // Üst kenar dördüncü derece: ortadaki sesten sızan iz (−100 dB'nin altı) kalmasın.
        let h = self.band_low.process(&mut self.band_state[0], h);
        let h = self.band_high.process(&mut self.band_state[1], h);
        self.band_high.process(&mut self.band_state[2], h)
    }

    /// Bir karenin bütün kanallarını yerinde işler.
    #[inline]
    pub fn process_frame(&mut self, frame: &mut [Sample]) {
        if self.is_bypass() {
            return;
        }
        if self.update_left == 0 {
            self.step();
            self.update_left = self.update_frames;
        }
        self.update_left -= 1;
        if self.mix != self.mix_target && Self::ramp(&mut self.mix, self.mix_target, self.ramp_step)
        {
            self.boost_dirty = true;
        }
        if self.depth != self.depth_target
            && Self::ramp(&mut self.depth, self.depth_target, self.ramp_step)
        {
            self.boost_dirty = true;
        }
        if self.punch_amount != self.punch_target
            && Self::ramp(&mut self.punch_amount, self.punch_target, self.ramp_step)
        {
            self.boost_dirty = true;
        }
        // Vuruş önce: bas düğmesi, alt oktav ve harmonikler vuruşlu bası işler.
        if self.punch_amount > 0.0 {
            self.punch.process_frame(frame, self.punch_amount);
        }

        let small = self.mix > 0.0;
        // Alt oktav küçük hoparlörde süzülür (çalınamaz); geçişte karışımla azalır.
        let deep = self.depth > 0.0 && self.mix < 1.0;
        let (harmonics, octave) = if small || deep {
            let mono = frame.iter().sum::<Sample>() / frame.len().max(1) as f64;
            let harmonics = if small { self.harmonics(mono) } else { 0.0 };
            let octave = if deep {
                self.octaver.process(mono) * self.depth * (1.0 - self.mix)
            } else {
                0.0
            };
            (harmonics, octave)
        } else {
            (0.0, 0.0)
        };
        for (channel, x) in frame.iter_mut().enumerate() {
            let Some(state) = self.shelf_state.get_mut(channel) else {
                continue;
            };
            let y = self.shelf.process(state, *x) + octave;
            *x = if small {
                let [first, second] = &mut self.sub_state[channel];
                let cut = self.sub_cut.process(first, y);
                let cut = self.sub_cut.process(second, cut) + harmonics;
                y + (cut - y) * self.mix
            } else {
                y
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::eq::EqSettings;
    use crate::audio::test_util::{fit, sine};

    const RATE: u32 = 48_000;

    fn control(bass_db: f64, small_speaker: bool) -> Arc<EqControl> {
        deep(bass_db, 0.0, small_speaker)
    }

    fn deep(bass_db: f64, bass_depth: f64, small_speaker: bool) -> Arc<EqControl> {
        punchy(bass_db, bass_depth, 0.0, small_speaker)
    }

    fn punchy(bass_db: f64, bass_depth: f64, bass_punch: f64, small: bool) -> Arc<EqControl> {
        Arc::new(EqControl::new(EqSettings {
            bass_db,
            small_speaker: small,
            bass_depth,
            bass_punch,
            ..EqSettings::default()
        }))
    }

    /// Örnekten örneğe işler (ilk yarım saniye atılır).
    fn run_signal(processor: &mut BassProcessor, signal: impl Fn(usize) -> f64) -> Vec<Sample> {
        let frames = RATE as usize * 2;
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            if f % 480 == 0 {
                processor.begin_block();
            }
            let mut frame = [signal(f)];
            processor.process_frame(&mut frame);
            out.push(frame[0]);
        }
        out.split_off(RATE as usize / 2)
    }

    fn peak(signal: &[Sample]) -> f64 {
        signal.iter().fold(0.0f64, |m, x| m.max(x.abs()))
    }

    /// `freq`'te `amplitude` genlikli tek kanallı sinüsü işler; ilk yarım saniye atılır.
    fn run(processor: &mut BassProcessor, freq: f64, amplitude: f64) -> Vec<Sample> {
        let frames = RATE as usize * 2;
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            if f % 480 == 0 {
                processor.begin_block();
            }
            let mut frame = [amplitude * sine(freq, RATE, f)];
            processor.process_frame(&mut frame);
            out.push(frame[0]);
        }
        out.split_off(RATE as usize / 2)
    }

    fn level_db(signal: &[Sample], freq: f64) -> f64 {
        20.0 * fit(signal, freq, RATE).0.log10()
    }

    #[test]
    fn duz_ayarda_ses_hic_degismez() {
        let mut processor = BassProcessor::new(control(0.0, false), 1, RATE);
        assert!(processor.is_bypass());
        let mut frame = [0.123_456_789];
        processor.process_frame(&mut frame);
        assert_eq!(frame[0], 0.123_456_789);
        assert_eq!(processor.boost().rise_db(None), 0.0);
    }

    #[test]
    fn bas_dugmesi_basi_yukseltir_ortayi_birakir() {
        let mut processor = BassProcessor::new(control(9.0, false), 1, RATE);
        let input_db = 20.0 * 0.1f64.log10();
        let bass = level_db(&run(&mut processor, 40.0, 0.1), 40.0) - input_db;
        assert!((bass - 9.0).abs() < 0.5, "40 Hz: {bass:.2} dB");
        let mid = level_db(&run(&mut processor, 1000.0, 0.1), 1000.0) - input_db;
        assert!(mid.abs() < 0.05, "1 kHz: {mid:.2} dB");
        // Eğri de aynı şeyi söyler.
        assert!((response_db(9.0, false, 40.0, 48_000.0) - bass).abs() < 0.1);
        assert_eq!(processor.boost().rise_db(None), 9.0);
    }

    #[test]
    fn bas_dugmesi_bozulma_eklemez() {
        let mut processor = BassProcessor::new(control(12.0, false), 1, RATE);
        for freq in [50.0, 1000.0] {
            let (_, noise_db) = fit(&run(&mut processor, freq, 0.1), freq, RATE);
            assert!(noise_db < -140.0, "{freq} Hz: {noise_db:.1} dB");
        }
    }

    #[test]
    fn kucuk_hoparlorde_harmonikler_eklenir_alt_bas_suzulur() {
        let mut processor = BassProcessor::new(control(12.0, true), 1, RATE);
        let freq = 50.0;
        let out = run(&mut processor, freq, 0.1);
        let fundamental = level_db(&out, freq);
        let second = level_db(&out, 2.0 * freq);
        let third = level_db(&out, 3.0 * freq);
        let input_db = 20.0 * 0.1f64.log10();
        // 100 ve 150 Hz: küçük hoparlörün çalabildiği harmonikler belirgin.
        assert!(
            second - input_db > -6.0,
            "2. harmonik {:.1} dB",
            second - input_db
        );
        assert!(
            third - input_db > -12.0,
            "3. harmonik {:.1} dB",
            third - input_db
        );
        // 50 Hz'in kendisi raf yükseltmesine rağmen süzülür (hoparlör zaten çalamaz).
        assert!(
            fundamental - input_db < 9.0,
            "temel {:.1} dB",
            fundamental - input_db
        );
        assert!(second > fundamental - 6.0, "harmonik baskın");
    }

    #[test]
    fn kucuk_hoparlor_ortadaki_sese_dokunmaz() {
        // 1 kHz'te üretilen harmonik yok denecek kadar az: ses temiz kalır.
        let mut processor = BassProcessor::new(control(12.0, true), 1, RATE);
        let (amplitude, noise_db) = fit(&run(&mut processor, 1000.0, 0.3), 1000.0, RATE);
        assert!((20.0 * (amplitude / 0.3).log10()).abs() < 0.1);
        // Ölçülen ~−98 dB: 16 bitlik kaydın (CD) kendi gürültüsü düzeyinde, duyulmaz.
        assert!(noise_db < -95.0, "1 kHz: {noise_db:.1} dB");
    }

    #[test]
    fn yukseltme_payi_tepeleri_karsilar() {
        // En kötü durum: en yüksek bas ve derinlik, küçük hoparlör; bas notaları ve ortada ses.
        for small in [false, true] {
            let mut processor = BassProcessor::new(deep(MAX_BASS_DB, 1.0, small), 1, RATE);
            let allowed = 10f64.powf(processor.boost().rise_db(None) / 20.0);
            for freq in [30.0, 45.0, 60.0, 80.0, 120.0, 200.0] {
                let input = 0.05;
                let peak = run(&mut processor, freq, input)
                    .iter()
                    .fold(0.0f64, |m, x| m.max(x.abs()));
                assert!(
                    peak <= input * allowed * 1.001,
                    "{freq} Hz, küçük hoparlör {small}: {:.2} dB > {:.2} dB",
                    20.0 * (peak / input).log10(),
                    processor.boost().rise_db(None)
                );
            }
        }
    }

    #[test]
    fn derinlik_bir_oktav_alti_ekler() {
        let input_db = 20.0 * 0.1f64.log10();
        // 80 Hz bas notası: 40 Hz'te alt oktav belirgin.
        let mut processor = BassProcessor::new(deep(0.0, 1.0, false), 1, RATE);
        let out = run(&mut processor, 80.0, 0.1);
        let octave = level_db(&out, 40.0) - input_db;
        assert!(octave > -6.0, "40 Hz: {octave:.1} dB");
        // Asıl nota yerinde kalır.
        assert!((level_db(&out, 80.0) - input_db).abs() < 1.0);
        // Derinlik kapalıyken alt oktav yok.
        let mut flat = BassProcessor::new(deep(0.0, 0.0, false), 1, RATE);
        let out = run(&mut flat, 80.0, 0.1);
        assert!(level_db(&out, 40.0) - input_db < -100.0);
    }

    #[test]
    fn derinlik_ortadaki_sese_dokunmaz() {
        let mut processor = BassProcessor::new(deep(0.0, 1.0, false), 1, RATE);
        let (amplitude, noise_db) = fit(&run(&mut processor, 1000.0, 0.3), 1000.0, RATE);
        assert!((20.0 * (amplitude / 0.3).log10()).abs() < 0.1);
        assert!(noise_db < -95.0, "1 kHz: {noise_db:.1} dB");
    }

    #[test]
    fn kucuk_hoparlorde_alt_oktav_eklenmez() {
        // Dizüstü hoparlörü 40 Hz'i çalamaz: derinlik açık olsa da alt oktav süzülür.
        let mut processor = BassProcessor::new(deep(0.0, 1.0, true), 1, RATE);
        let out = run(&mut processor, 80.0, 0.1);
        let input_db = 20.0 * 0.1f64.log10();
        assert!(level_db(&out, 40.0) - input_db < -20.0);
    }

    #[test]
    fn derinlik_acilinca_tik_olmaz() {
        let control = deep(0.0, 0.0, false);
        let mut processor = BassProcessor::new(Arc::clone(&control), 1, RATE);
        let quarter = RATE as usize / 4;
        let mut previous = 0.0;
        let (mut during, mut settled): (f64, f64) = (0.0, 0.0);
        for f in 0..RATE as usize {
            if f % 480 == 0 {
                processor.begin_block();
            }
            if f == quarter {
                control.set(EqSettings {
                    bass_db: 15.0,
                    bass_depth: 1.0,
                    ..EqSettings::default()
                });
            }
            let mut frame = [0.2 * sine(70.0, RATE, f)];
            processor.process_frame(&mut frame);
            let jump = (frame[0] - previous).abs();
            previous = frame[0];
            if (quarter..2 * quarter).contains(&f) {
                during = during.max(jump);
            } else if f >= 3 * quarter {
                settled = settled.max(jump);
            }
        }
        assert!(
            during <= settled * 1.1,
            "geçişte {during:.4}, oturunca {settled:.4}"
        );
    }

    #[test]
    fn olculen_bas_tepeleri_gercek_artisi_karsilar_ve_gereksiz_kismaz() {
        // Tipik bir kayıt: tepe ortadaki sesten (vokal, davul) gelir, bas daha kısık.
        let signal = |f: usize| 0.15 * sine(55.0, RATE, f) + 0.6 * sine(1000.0, RATE, f);
        let mut meter = BassPeakMeter::new(RATE, 1);
        for f in 0..RATE as usize * 2 {
            meter.add(&[signal(f)]);
        }
        let peaks = meter.finish().unwrap();
        // Raf arttıkça tepe artışı da artar.
        assert!(peaks.shelf_rise_db.windows(2).all(|w| w[0] <= w[1]));
        assert!(peaks.octave_band_db < -10.0, "{}", peaks.octave_band_db);
        let input_peak = peak(&(0..RATE as usize).map(signal).collect::<Vec<_>>());
        for (bass_db, depth) in [
            (6.0, 0.0),
            (12.0, 0.0),
            (18.0, 0.0),
            (12.0, 1.0),
            (18.0, 0.7),
        ] {
            let mut processor = BassProcessor::new(deep(bass_db, depth, false), 1, RATE);
            let rise = linear_to_db(peak(&run_signal(&mut processor, signal)) / input_peak);
            let predicted = processor.boost().rise_db(Some(&peaks));
            let worst = processor.boost().rise_db(None);
            assert!(
                rise <= predicted + 0.1,
                "bas {bass_db}, derinlik {depth}: gerçek {rise:.2} > tahmin {predicted:.2}"
            );
            // Ölçüm, en kötü duruma göre çok daha az kısar.
            assert!(
                predicted < worst - 3.0,
                "bas {bass_db}: ölçülen {predicted:.2}, en kötü {worst:.2}"
            );
        }
        // Sessizlik ölçülmez.
        assert_eq!(BassPeakMeter::new(RATE, 2).finish(), None);
        // Ara değerler doğrusal ve güvenlik paylı.
        let p = BassPeaks {
            shelf_rise_db: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            octave_band_db: -6.0,
        };
        assert_eq!(p.shelf_rise_db(0.0), 0.0);
        assert!((p.shelf_rise_db(1.5) - (0.5 + MEASURED_MARGIN_DB)).abs() < 1e-9);
        assert!((p.shelf_rise_db(10.5) - (3.5 + MEASURED_MARGIN_DB)).abs() < 1e-9);
        assert!((p.shelf_rise_db(30.0) - (9.0 + MEASURED_MARGIN_DB)).abs() < 1e-9);
    }

    #[test]
    fn kucuk_hoparlorde_koruma_gereksiz_kismaz() {
        // Alt bas süzüldüğü için rafın tamamı tepeye eklenmez (ölçülen: bas 8'de ~3 dB).
        // Fazla koruma, dizüstünde sesi gereksiz kısardı.
        assert!(
            boost_db(8.0, 0.0, 0.0, true) < 5.0,
            "{}",
            boost_db(8.0, 0.0, 0.0, true)
        );
        assert!(
            boost_db(12.0, 0.0, 0.0, true) < 7.0,
            "{}",
            boost_db(12.0, 0.0, 0.0, true)
        );
        assert_eq!(boost_db(8.0, 0.0, 0.0, false), 8.0);
        let processor = BassProcessor::new(control(8.0, true), 1, RATE);
        assert_eq!(
            processor.boost().rise_db(None),
            boost_db(8.0, 0.0, 0.0, true)
        );
    }

    #[test]
    fn ayar_degisikligi_yumusakca_uygulanir() {
        let control = control(0.0, false);
        let mut processor = BassProcessor::new(Arc::clone(&control), 1, RATE);
        let quarter = RATE as usize / 4;
        let mut previous = 0.0;
        // Örnekten örneğe en büyük adım: geçiş sırasında ve ayar oturduktan sonra.
        let (mut during, mut settled): (f64, f64) = (0.0, 0.0);
        for f in 0..RATE as usize {
            if f % 480 == 0 {
                processor.begin_block();
            }
            if f == quarter {
                control.set(EqSettings {
                    bass_db: 12.0,
                    small_speaker: true,
                    ..EqSettings::default()
                });
            }
            let mut frame = [0.2 * sine(60.0, RATE, f)];
            processor.process_frame(&mut frame);
            let jump = (frame[0] - previous).abs();
            previous = frame[0];
            if (quarter..2 * quarter).contains(&f) {
                during = during.max(jump);
            } else if f >= 3 * quarter {
                settled = settled.max(jump);
            }
        }
        // Tık, sesin kendi adımlarından çok büyük bir sıçramadır.
        assert!(
            during <= settled * 1.1,
            "geçişte {during:.4}, oturunca {settled:.4}"
        );
    }

    #[test]
    fn olculen_kazanclar_artarak_sirali() {
        assert!(PEAK_STEPS_DB.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn vurus_koruma_payi_tepeleri_karsilar() {
        use crate::audio::punch::tests::kick;
        // Tipik kayıt: davul vuruşları, sürekli bas ve ortada ses (vokal).
        let signal =
            |f: usize| 0.35 * kick(f) + 0.1 * sine(55.0, RATE, f) + 0.35 * sine(1000.0, RATE, f);
        let mut meter = BassPeakMeter::new(RATE, 1);
        for f in 0..RATE as usize * 2 {
            meter.add(&[signal(f)]);
        }
        let peaks = meter.finish().unwrap();
        let input_peak = peak(&(0..RATE as usize * 2).map(signal).collect::<Vec<_>>());
        for (bass_db, punch, small) in [
            (0.0, 1.0, false),
            (6.0, 0.5, false),
            (14.0, 0.7, false),
            (18.0, 1.0, false),
            (8.0, 1.0, true),
        ] {
            let mut processor = BassProcessor::new(punchy(bass_db, 0.0, punch, small), 1, RATE);
            let rise = linear_to_db(peak(&run_signal(&mut processor, signal)) / input_peak);
            let worst = processor.boost().rise_db(None);
            assert!(
                rise <= worst + 0.01,
                "bas {bass_db}, vuruş {punch}, küçük {small}: gerçek {rise:.2} > en kötü {worst:.2}"
            );
            if !small {
                let predicted = processor.boost().rise_db(Some(&peaks));
                assert!(
                    rise <= predicted + 0.1,
                    "bas {bass_db}, vuruş {punch}: gerçek {rise:.2} > ölçülen {predicted:.2}"
                );
            }
        }
        // Vuruş yalnızca ilk an; koruma ona göre: vuruş %100 = 8 dB'lik raf.
        assert_eq!(boost_db(10.0, 0.0, 1.0, false), 10.0 + PUNCH_MAX_DB);
        assert_eq!(boost_db(10.0, 0.0, 0.5, false), 10.0 + PUNCH_MAX_DB / 2.0);
    }

    #[test]
    fn vurus_acilinca_tik_olmaz_surekli_bas_degismez() {
        let control = control(0.0, false);
        let mut processor = BassProcessor::new(Arc::clone(&control), 1, RATE);
        let quarter = RATE as usize / 4;
        let mut previous = 0.0;
        let (mut during, mut settled): (f64, f64) = (0.0, 0.0);
        let mut tail = Vec::new();
        for f in 0..RATE as usize * 2 {
            if f % 480 == 0 {
                processor.begin_block();
            }
            if f == quarter {
                control.set(EqSettings {
                    bass_punch: 1.0,
                    ..EqSettings::default()
                });
            }
            let mut frame = [0.3 * sine(55.0, RATE, f)];
            processor.process_frame(&mut frame);
            let jump = (frame[0] - previous).abs();
            previous = frame[0];
            if (quarter..2 * quarter).contains(&f) {
                during = during.max(jump);
            } else if f >= 3 * quarter {
                settled = settled.max(jump);
            }
            if f >= RATE as usize {
                tail.push(frame[0]);
            }
        }
        assert!(
            during <= settled * 1.01,
            "geçişte {during:.4}, oturunca {settled:.4}"
        );
        // Sürekli bas (bas gitar) vuruş açıkken de aynen geçer.
        let (amplitude, noise_db) = fit(&tail, 55.0, RATE);
        assert!((linear_to_db(amplitude / 0.3)).abs() < 0.05);
        assert!(noise_db < -100.0, "bozulma {noise_db:.1} dB");
    }

    #[test]
    fn ekolayzer_kapaliyken_bas_da_kapanir() {
        let control = Arc::new(EqControl::new(EqSettings {
            enabled: false,
            bass_db: 12.0,
            small_speaker: true,
            ..EqSettings::default()
        }));
        let processor = BassProcessor::new(control, 2, RATE);
        assert!(processor.is_bypass());
    }
}
