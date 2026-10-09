//! Vuruş: davul vuruşlarının (kick) ilk anını güçlendiren dinamik bas rafı.
//!
//! Bas bandında (150 Hz altı) iki zarf izlenir:
//!
//! - **Hızlı zarf** (tepe tutucu) vuruşun ilk dalgasında anında yükselir; en alt basın
//!   yarım dalgası boyunca (20 ms) tepeyi tutar, sürekli basta dalgalanmaz.
//! - **Gecikmeli zarf** hızlıyı ~30 ms gecikmeyle izleyerek yükselir, inişte onunla
//!   birlikte iner.
//!
//! Hızlı zarf gecikmelinin üstünde kaldığı sürece (vuruşun ilk ~50 ms'si) 100 Hz alçak raf
//! yükselir, sonra yumuşakça iner. Sürekli basta (bas gitar, uzun notalar) iki zarf eşittir:
//! raf düz kalır, ses değişmez. Bas şişmez, çamurlaşmaz; yalnızca vuruş göğse çarpar.
//!
//! Raf kazancı yavaş değişen bir zarftır: süzgeç her [`UPDATE_FRAMES`] örnekte yeniden
//! hesaplanır. Açılınca algılayıcı önce şarkının bas düzeyini öğrenir (yanlış vuruş
//! sanılmasın). Bellek yalnızca kurulurken ayrılır.

use std::f64::consts::FRAC_1_SQRT_2;

use super::biquad::{self, Biquad, State};
use super::Sample;

/// Vuruşun en büyük raf kazancı (dB, "Vuruş" %100'de).
pub const PUNCH_MAX_DB: f64 = 8.0;
/// Rafın köşe frekansı (Hz): bas düğmesinin rafıyla aynı (taşma koruması ikisini birlikte
/// tek raf gibi hesaplar).
pub const PUNCH_SHELF_HZ: f64 = 100.0;
/// Algılanan bas bandının üst sınırı (Hz; dördüncü derece).
const DETECT_HZ: f64 = 150.0;
/// Hızlı zarfın tepeyi tutma süresi (s): 25 Hz'e kadar basın yarım dalgası; tepeler
/// arasında zarf inmez, kazanç dalgalanmaz.
const FAST_HOLD_SECONDS: f64 = 0.02;
/// Tutulan tepeye bu kadar yakın (−1 dB) her tepe tutmayı tazeler: sürekli basın ardışık
/// tepeleri örneklemeden ötürü biraz farklı olsa da zarf arada düşmez.
const HOLD_REFRESH: f64 = 0.891;
/// Hızlı zarfın tutma bittikten sonraki iniş zaman sabiti (s).
const FAST_RELEASE_SECONDS: f64 = 0.03;
/// Gecikmeli zarfın yükselme zaman sabiti (s): vuruşun güçlendirilen ilk anının uzunluğu.
const SLOW_ATTACK_SECONDS: f64 = 0.03;
/// Hızlı zarfın yavaşı bu kadar (dB) aşmadıkça vuruş sayılmaz: sürekli basın küçük
/// dalgalanmaları rafı oynatmaz.
const DEADBAND_DB: f64 = 1.0;
/// Vuruşun her dB'si için rafın yükselmesi (dB).
const SLOPE: f64 = 1.0;
/// Kazancın yükselme ve inme zaman sabitleri (s).
const GAIN_ATTACK_SECONDS: f64 = 0.002;
const GAIN_RELEASE_SECONDS: f64 = 0.04;
/// Bu düzeyin (−80 dB) altındaki bas vuruş sayılmaz: sessizlikte gürültü oynamasın.
const SILENCE: f64 = 1e-4;
/// Raf bu kadar örnekte bir yeniden hesaplanır.
pub const UPDATE_FRAMES: usize = 16;
/// Kazanç bu kadar (dB) değişmedikçe süzgeç yeniden hesaplanmaz.
const RECOMPUTE_DB: f64 = 0.02;
/// Açılınca algılayıcının bas düzeyini öğrendiği süre (s): bu sürede raf düz kalır.
const WARMUP_SECONDS: f64 = 0.15;

/// Vuruş işlemcisi. `amount` (0..1) "Vuruş" düğmesidir; çağıran yumuşakça değiştirir.
pub struct Punch {
    rate: f64,
    detect: Biquad,
    detect_state: [State; 2],
    fast: f64,
    hold_frames: usize,
    hold_left: usize,
    fast_release: f64,
    slow: f64,
    slow_coef: f64,
    attack_coef: f64,
    release_coef: f64,
    /// Şu anki raf kazancı (dB, vuruş %100'e göre) ve süzgece uygulanmış olanı.
    gain_db: f64,
    applied_db: f64,
    shelf: Biquad,
    shelf_state: Vec<State>,
    update_left: usize,
    warmup_frames: usize,
    warmup_left: usize,
}

impl Punch {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let block = UPDATE_FRAMES as f64;
        Self {
            rate,
            detect: Biquad::lowpass(DETECT_HZ, FRAC_1_SQRT_2, rate),
            detect_state: [[0.0; 2]; 2],
            fast: 0.0,
            hold_frames: (FAST_HOLD_SECONDS * rate) as usize,
            hold_left: 0,
            fast_release: (-1.0 / (FAST_RELEASE_SECONDS * rate)).exp(),
            slow: 0.0,
            slow_coef: 1.0 - (-1.0 / (SLOW_ATTACK_SECONDS * rate)).exp(),
            attack_coef: 1.0 - (-block / (GAIN_ATTACK_SECONDS * rate)).exp(),
            release_coef: 1.0 - (-block / (GAIN_RELEASE_SECONDS * rate)).exp(),
            gain_db: 0.0,
            applied_db: 0.0,
            shelf: Biquad::IDENTITY,
            shelf_state: vec![[0.0; 2]; channels.max(1)],
            update_left: 0,
            warmup_frames: (WARMUP_SECONDS * rate) as usize,
            warmup_left: 0,
        }
    }

    /// Algılayıcıyı baştan başlatır (vuruş kapalıyken açılınca): önce şarkının bas düzeyini
    /// öğrenir, bu sürede raf düz kalır.
    pub fn restart(&mut self) {
        for state in self
            .detect_state
            .iter_mut()
            .chain(self.shelf_state.iter_mut())
        {
            *state = [0.0; 2];
        }
        self.fast = 0.0;
        self.hold_left = 0;
        self.slow = 0.0;
        self.gain_db = 0.0;
        self.applied_db = 0.0;
        self.shelf = Biquad::IDENTITY;
        self.update_left = 0;
        self.warmup_left = self.warmup_frames;
    }

    /// Şu anki raf kazancı (dB, vuruş %100'e göre; testler ve gösterge için).
    pub fn gain_db(&self) -> f64 {
        self.gain_db
    }

    /// Çok küçük (denormal) değerleri sıfırlar (her doldurma turunda bir).
    pub fn flush(&mut self) {
        for state in self
            .detect_state
            .iter_mut()
            .chain(self.shelf_state.iter_mut())
        {
            biquad::flush(state);
        }
    }

    /// Bir karenin bütün kanallarını yerinde işler; `amount` (0..1) vuruşun miktarı.
    #[inline]
    pub fn process_frame(&mut self, frame: &mut [Sample], amount: f64) {
        let mono = frame.iter().sum::<Sample>() / frame.len().max(1) as f64;
        let low = self.detect.process(&mut self.detect_state[0], mono);
        let low = self.detect.process(&mut self.detect_state[1], low).abs();
        if low >= self.fast {
            self.fast = low;
            self.hold_left = self.hold_frames;
        } else if low >= self.fast * HOLD_REFRESH {
            self.hold_left = self.hold_frames;
        } else if self.hold_left > 0 {
            self.hold_left -= 1;
        } else {
            self.fast *= self.fast_release;
        }
        if self.warmup_left > 0 {
            // Öğrenirken yavaş zarf hızlıyı doğrudan izler: hiçbir şey vuruş sayılmaz.
            self.warmup_left -= 1;
            self.slow = self.slow.max(self.fast);
        } else if self.fast > self.slow {
            self.slow += (self.fast - self.slow) * self.slow_coef;
        } else {
            // İnişte gecikme yok: vuruş bitince yükseltme de biter, gövde şişmez.
            self.slow = self.fast;
        }

        if self.update_left == 0 {
            self.update_left = UPDATE_FRAMES;
            self.update_gain(amount);
        }
        self.update_left -= 1;

        for (x, state) in frame.iter_mut().zip(self.shelf_state.iter_mut()) {
            *x = self.shelf.process(state, *x);
        }
    }

    /// Vuruşun kazancını zarflardan hesaplar, yumuşatır; gerekiyorsa rafı yeniler.
    fn update_gain(&mut self, amount: f64) {
        let target = if self.fast > SILENCE && self.warmup_left == 0 {
            let rise_db = 20.0 * (self.fast / self.slow.max(SILENCE)).log10();
            (SLOPE * (rise_db - DEADBAND_DB)).clamp(0.0, PUNCH_MAX_DB)
        } else {
            0.0
        };
        let coef = if target > self.gain_db {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.gain_db += (target - self.gain_db) * coef;
        if target == 0.0 && self.gain_db < RECOMPUTE_DB {
            self.gain_db = 0.0;
        }
        let wanted = self.gain_db * amount.clamp(0.0, 1.0);
        // Düz rafa (0 dB) her zaman tam olarak dönülür: ses aynen geçer.
        if (wanted - self.applied_db).abs() >= RECOMPUTE_DB
            || (wanted == 0.0) != (self.applied_db == 0.0)
        {
            self.applied_db = wanted;
            self.shelf = Biquad::low_shelf(PUNCH_SHELF_HZ, wanted, FRAC_1_SQRT_2, self.rate);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::audio::test_util::{fit, sine};

    const RATE: u32 = 48_000;
    /// Vuruşlar arası süre (örnek; yarım saniye: 120 BPM).
    pub(crate) const PERIOD: usize = RATE as usize / 2;

    /// Sentetik davul vuruşu: perdesi ~110 Hz'ten 50 Hz'e hızla inen, ~90 ms'de sönen sinüs;
    /// her `PERIOD` örnekte bir.
    pub(crate) fn kick(f: usize) -> f64 {
        let t = (f % PERIOD) as f64 / f64::from(RATE);
        let phase = 2.0 * std::f64::consts::PI * (50.0 * t + 1.8 * (1.0 - (-t / 0.03).exp()));
        phase.sin() * (-t / 0.09).exp()
    }

    fn run(amount: f64, signal: impl Fn(usize) -> f64, seconds: usize) -> Vec<Sample> {
        let mut punch = Punch::new(1, RATE);
        (0..RATE as usize * seconds)
            .map(|f| {
                if f % 480 == 0 {
                    punch.flush();
                }
                let mut frame = [signal(f)];
                if amount > 0.0 {
                    punch.process_frame(&mut frame, amount);
                }
                frame[0]
            })
            .collect()
    }

    fn rms(signal: &[Sample]) -> f64 {
        (signal.iter().map(|x| x * x).sum::<f64>() / signal.len().max(1) as f64).sqrt()
    }

    /// Vuruşların (ilk saniyeden sonra) ilk 40 ms'sinin ve 120–250 ms arasının RMS'i.
    fn attack_and_body(out: &[Sample]) -> (f64, f64) {
        let ms = |m: usize| m * RATE as usize / 1000;
        let (mut attack, mut body) = (Vec::new(), Vec::new());
        for start in (RATE as usize..out.len() - PERIOD).step_by(PERIOD) {
            attack.extend_from_slice(&out[start..start + ms(40)]);
            body.extend_from_slice(&out[start + ms(120)..start + ms(250)]);
        }
        (rms(&attack), rms(&body))
    }

    fn db(ratio: f64) -> f64 {
        20.0 * ratio.log10()
    }

    #[test]
    fn vurusun_ilk_anini_guclendirir_govdesini_sisirmez() {
        let signal = |f| 0.5 * kick(f);
        let (dry_attack, dry_body) = attack_and_body(&run(0.0, signal, 6));
        let (wet_attack, wet_body) = attack_and_body(&run(1.0, signal, 6));
        let attack = db(wet_attack / dry_attack);
        let body = db(wet_body / dry_body);
        // Ölçülen: ilk 40 ms +4,6 dB, gövde +0,5 dB.
        assert!(attack > 4.0, "vuruşun ilk anı {attack:.2} dB");
        assert!(body < 1.0, "gövde {body:.2} dB");
        // Yarım vuruş, yarısı kadar.
        let (half_attack, _) = attack_and_body(&run(0.5, signal, 6));
        let half = db(half_attack / dry_attack);
        assert!(
            half > 1.5 && half < attack - 1.5,
            "yarım vuruş {half:.2} dB"
        );
    }

    #[test]
    fn bas_cizgisi_ustundeki_vurusu_da_guclendirir() {
        // Sürekli bir bas çizgisinin (55 Hz) üstünde vuruşlar: vuruş yine öne çıkar.
        let signal = |f| 0.25 * sine(55.0, RATE, f) + 0.5 * kick(f);
        let (dry_attack, _) = attack_and_body(&run(0.0, signal, 6));
        let (wet_attack, _) = attack_and_body(&run(1.0, signal, 6));
        let attack = db(wet_attack / dry_attack);
        // Ölçülen: +2,3 dB.
        assert!(attack > 2.0, "vuruşun ilk anı {attack:.2} dB");
    }

    #[test]
    fn surekli_basa_dokunmaz() {
        // Bas gitar gibi sürekli bir nota: öğrenme bitince raf düz, ses aynen geçer.
        for freq in [35.0, 55.0, 90.0] {
            let out = run(1.0, |f| 0.3 * sine(freq, RATE, f), 3);
            let tail = &out[RATE as usize..];
            let (amplitude, noise_db) = fit(tail, freq, RATE);
            assert!(
                db(amplitude / 0.3).abs() < 0.05,
                "{freq} Hz: {:.3} dB",
                db(amplitude / 0.3)
            );
            assert!(noise_db < -100.0, "{freq} Hz bozulma {noise_db:.1} dB");
        }
    }

    #[test]
    fn ortadaki_sese_dokunmaz() {
        // Vuruşlar sürerken 1 kHz'teki ses (vokal) yerinde kalır.
        let signal = |f| 0.5 * kick(f) + 0.2 * sine(1000.0, RATE, f);
        let dry = run(0.0, signal, 4);
        let wet = run(1.0, signal, 4);
        let level = |out: &[Sample]| fit(&out[RATE as usize..], 1000.0, RATE).0;
        let change = db(level(&wet) / level(&dry));
        assert!(change.abs() < 0.1, "1 kHz {change:.3} dB");
    }

    #[test]
    fn acilinca_bas_duzeyini_ogrenir_tik_ve_sisme_olmaz() {
        // Sürekli bas çalarken vuruş açılır: öğrenme süresince raf düz kalır, ses sıçramaz.
        let mut punch = Punch::new(1, RATE);
        let quarter = RATE as usize / 4;
        let mut peak_after: f64 = 0.0;
        for f in 0..RATE as usize * 2 {
            if f == quarter {
                punch.restart();
            }
            let mut frame = [0.3 * sine(55.0, RATE, f)];
            if f >= quarter {
                punch.process_frame(&mut frame, 1.0);
                peak_after = peak_after.max(frame[0].abs());
            }
        }
        assert!(
            peak_after <= 0.3 * 1.001,
            "açılınca {:.3} dB",
            db(peak_after / 0.3)
        );
        assert_eq!(punch.gain_db(), 0.0);
    }

    #[test]
    fn sessizlikte_bir_sey_yapmaz() {
        let out = run(1.0, |f| 1e-6 * kick(f), 2);
        let dry = run(0.0, |f| 1e-6 * kick(f), 2);
        assert_eq!(out, dry);
    }
}
