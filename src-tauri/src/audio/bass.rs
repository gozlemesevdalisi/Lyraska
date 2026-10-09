//! Bas motoru: "Bas" düğmesi ve küçük hoparlörler için psikoakustik bas.
//!
//! - **Bas düğmesi** (0–12 dB): 100 Hz alçak raf süzgeci. Grafik ekolayzerden bağımsız,
//!   tek dokunuşla bas. Yükseltme, ses yüksekliği eşitlemesinin tepeye açtığı boşluktan
//!   karşılanır ([`super::normalize`]); boşluk yetmezse yalnızca eksik kadar kısılır.
//! - **Küçük hoparlör bası**: dizüstü ve küçük hoparlörler ~100 Hz'in altını çalamaz;
//!   oradaki bası yükseltmek yalnızca bozulma yaratır. Bunun yerine basın 2., 3. ve 4.
//!   harmonikleri üretilip eklenir: beyin, duyulmayan temel notayı harmoniklerden
//!   tamamlar ("eksik temel" etkisi). Hoparlörün çalamadığı alt bas da süzülür.
//!
//! Harmonikler Chebyshev polinomlarıyla üretilir: genliği 1'e getirilmiş bir sinüste
//! `T_n(cos θ) = cos(nθ)`, yani n. harmonik tam olarak ve istenen oranda çıkar. Genlik
//! zarfla geri verildiği için harmoniklerin yüksekliği basın yüksekliğini izler.
//!
//! Ayarlar ekolayzerin kilitsiz kanalından ([`EqControl`]) okunur; değişiklikler ~40 ms'de
//! yumuşakça uygulanır. Bas 0'da ve küçük hoparlör kapalıyken ses hiç işlenmez. Bellek
//! yalnızca kurulurken ayrılır.

use std::f64::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use super::biquad::{self, Biquad, State};
use super::eq::EqControl;
use super::Sample;

/// Bas düğmesinin üst sınırı (dB).
pub const MAX_BASS_DB: f64 = 12.0;
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
/// taban, bas düğmesiyle tavana kadar artar.
const HARMONIC_BASE: f64 = 0.5;
const HARMONIC_MAX: f64 = 1.5;
/// Bas zarfının iniş zaman sabiti (s). Zarf anında yükselir: bas birden başlasa da
/// harmonik üretici doyuma girmez (kare dalga, tık olmaz).
const ENVELOPE_RELEASE_SECONDS: f64 = 0.15;
/// Ayar değişikliklerinin yumuşatılması: her 2 ms'de bir, ~40 ms'lik zaman sabitiyle.
const UPDATE_SECONDS: f64 = 0.002;
const SMOOTH_SECONDS: f64 = 0.040;
/// Hedefe bu kadar yaklaşınca doğrudan hedefe oturulur (dB).
const SNAP_DB: f64 = 0.01;
/// Küçük hoparlörde harmonikler için taşma korumasına eklenen pay (dB). Ölçülen: süzgeçlerin
/// en büyük artışının üstüne en fazla ~2,6 dB (`yukseltme_payi_tepeleri_karsilar` testi).
const HARMONIC_ALLOWANCE_DB: f64 = 3.0;

/// Düğmenin ayarından harmonik miktarı.
fn harmonic_amount(bass_db: f64) -> f64 {
    HARMONIC_BASE + (HARMONIC_MAX - HARMONIC_BASE) * (bass_db / MAX_BASS_DB).clamp(0.0, 1.0)
}

/// Bas motorunun doğrusal kısmının (raf ve alt bas süzgeci) `hz`'deki kazancı (dB).
/// Harmonikler doğrusal olmadığı için eğride gösterilmez.
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

/// Bas motorunun sese ekleyebileceği en büyük yükseltme (dB): taşma koruması buna göre.
///
/// Küçük hoparlörde alt bas süzüldüğü için rafın tamamı tepeye eklenmez: süzgeçlerin en
/// büyük artışı (30–300 Hz taranır) ve harmonikler için pay. Fazla pay sesi gereksiz
/// kısardı; dizüstünde ses yüksekliği değerlidir.
pub fn boost_db(bass_db: f64, small_speaker: bool) -> f64 {
    if !small_speaker {
        return bass_db.max(0.0);
    }
    let peak = (0..=48)
        .map(|i| 30.0 * 10f64.powf(f64::from(i) / 48.0))
        .map(|hz| response_db(bass_db, true, hz, REFERENCE_RATE))
        .fold(0.0, f64::max);
    peak + HARMONIC_ALLOWANCE_DB
}

/// Yükseltme hesabında kullanılan örnekleme hızı (alt frekanslarda yanıt hıza bağlı değil).
const REFERENCE_RATE: f64 = 48_000.0;

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
    mix_step: f64,
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
    update_frames: usize,
    update_left: usize,
    smooth_coef: f64,
    /// Şu anki en büyük yükseltme (dB); ayar değişince yeniden hesaplanır.
    boost: f64,
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
            // Karışım her örnekte ilerler: basamaklı geçiş "fermuar" sesi yapar.
            mix_step: 1.0 / (SMOOTH_SECONDS * rate),
            split: Biquad::lowpass(SPLIT_HZ, q, rate),
            split_state: [[0.0; 2]; 2],
            envelope: 0.0,
            release: (-1.0 / (ENVELOPE_RELEASE_SECONDS * rate)).exp(),
            band_low: Biquad::highpass(HARMONIC_LOW_HZ, q, rate),
            band_high: Biquad::lowpass(HARMONIC_HIGH_HZ, q, rate),
            band_state: [[0.0; 2]; 3],
            sub_cut: Biquad::highpass(SUB_CUT_HZ, q, rate),
            sub_state: vec![[[0.0; 2]; 2]; channels],
            update_frames,
            update_left: 0,
            smooth_coef: 1.0 - (-(update_frames as f64) / (SMOOTH_SECONDS * rate)).exp(),
            boost: 0.0,
            boost_dirty: true,
        };
        // İlk ayar yumuşatmadan uygulanır (şarkı zaten sessizlikten başlar).
        processor.begin_block();
        processor.shelf_db = processor.target_db;
        processor.mix = processor.mix_target;
        processor.shelf = Biquad::low_shelf(SHELF_HZ, processor.shelf_db, q, rate);
        processor.refresh_boost();
        processor
    }

    /// Ses şu an hiç işlenmiyor mu?
    pub fn is_bypass(&self) -> bool {
        self.shelf_db == 0.0 && self.target_db == 0.0 && self.mix == 0.0 && self.mix_target == 0.0
    }

    /// Şu anki en büyük yükseltme (dB). Gerçek zamanlı yolda her karede okunur; hesap
    /// yalnızca ayar değişirken yapılır.
    pub fn boost_db(&self) -> f64 {
        self.boost
    }

    /// Yükseltmeyi yeniden hesaplar: geçiş sürerken hedefle büyüğü; küçük hoparlöre geçiş
    /// bitmeden (iki yol karışırken) rafın tamamı.
    fn refresh_boost(&mut self) {
        let bass_db = self.shelf_db.max(self.target_db);
        let full = boost_db(bass_db, false);
        self.boost = if self.mix == 0.0 && self.mix_target == 0.0 {
            full
        } else if self.mix == 1.0 && self.mix_target == 1.0 {
            boost_db(bass_db, true)
        } else {
            full.max(boost_db(bass_db, true))
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
        if self.mix != self.mix_target {
            let delta = self.mix_target - self.mix;
            self.mix = if delta.abs() <= self.mix_step {
                self.boost_dirty = true;
                self.mix_target
            } else {
                self.mix + self.mix_step.copysign(delta)
            };
        }

        let small = self.mix > 0.0;
        let harmonics = if small {
            let mono = frame.iter().sum::<Sample>() / frame.len().max(1) as f64;
            self.harmonics(mono)
        } else {
            0.0
        };
        for (channel, x) in frame.iter_mut().enumerate() {
            let Some(state) = self.shelf_state.get_mut(channel) else {
                continue;
            };
            let y = self.shelf.process(state, *x);
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
        Arc::new(EqControl::new(EqSettings {
            bass_db,
            small_speaker,
            ..EqSettings::default()
        }))
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
        assert_eq!(processor.boost_db(), 0.0);
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
        assert_eq!(processor.boost_db(), 9.0);
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
        // En kötü durum: en yüksek bas, küçük hoparlör; bas notaları ve ortada ses.
        for small in [false, true] {
            let mut processor = BassProcessor::new(control(MAX_BASS_DB, small), 1, RATE);
            let allowed = 10f64.powf(processor.boost_db() / 20.0);
            for freq in [30.0, 45.0, 60.0, 80.0, 120.0, 200.0] {
                let input = 0.05;
                let peak = run(&mut processor, freq, input)
                    .iter()
                    .fold(0.0f64, |m, x| m.max(x.abs()));
                assert!(
                    peak <= input * allowed * 1.001,
                    "{freq} Hz, küçük hoparlör {small}: {:.2} dB > {:.2} dB",
                    20.0 * (peak / input).log10(),
                    processor.boost_db()
                );
            }
        }
    }

    #[test]
    fn kucuk_hoparlorde_koruma_gereksiz_kismaz() {
        // Alt bas süzüldüğü için rafın tamamı tepeye eklenmez (ölçülen: bas 8'de ~3 dB).
        // Fazla koruma, dizüstünde sesi gereksiz kısardı.
        assert!(boost_db(8.0, true) < 5.0, "{}", boost_db(8.0, true));
        assert!(boost_db(12.0, true) < 7.0, "{}", boost_db(12.0, true));
        assert_eq!(boost_db(8.0, false), 8.0);
        let processor = BassProcessor::new(control(8.0, true), 1, RATE);
        assert_eq!(processor.boost_db(), boost_db(8.0, true));
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
