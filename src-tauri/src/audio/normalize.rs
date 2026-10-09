//! Çalarken ses yüksekliği eşitlemesi ve ekolayzer için boşluk yönetimi.
//!
//! - [`LoudnessControl`]: kullanıcının ayarı (eşitleme açık mı); oynatıcı boyunca aynı.
//! - [`TrackLevels`]: bir oturumdaki şarkıların ses akışındaki başlangıç karesi ve ölçülen
//!   ses yüksekliği. Boşluksuz geçişte çözme iş parçacığı yeni şarkıyı ekler; analiz
//!   bitince oynatıcı ölçümü yazar. Hepsi atomiktir: ses iş parçacığı hiç beklemez.
//! - [`Leveler`]: ses iş parçacığında kazancı uygular. Şarkı değişince kazanç tam o
//!   karede değişmeye başlar.
//!
//! Ekolayzer bir bandı yükseltince ses taşmasın diye kısılması gerekir. Eşitleme şarkıyı
//! hedefe getirirken çoğu zaman yer açar (yüksek kaydedilmiş şarkılar kısılır): ön kazanç
//! yalnızca bu boşluğun yetmediği kadar düşürülür. Böylece bas +6 dediğinizde bas gerçekten
//! +6 dB artar, sesin geri kalanı kısılmaz.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};

use super::bass::{BassBoost, BassPeaks, PEAK_STEPS_DB};
use super::loudness::{Loudness, TARGET_LUFS};
use super::Sample;

/// Ölçüm gelmeden önce varsayılan şarkı yüksekliği (LUFS): günümüz kayıtlarının tipik
/// düzeyi. Ölçüm genellikle şarkı başlamadan biter; bitmezse kazanç ondan sonra yumuşakça
/// düzelir.
const ASSUMED_LUFS: f64 = -10.0;
/// Aynı anda tutulan şarkı sayısı (çalan, sıradakiler; çözme en fazla 2 sn önden gider).
const SLOTS: usize = 8;
/// Kazancın yaklaşma süreleri (saniye): şarkı sınırında kısa (tık olmasın), ölçüm sonradan
/// gelince ya da ayar değişince uzun (fark edilmesin).
const SWITCH_SECONDS: f64 = 0.1;
const UPDATE_SECONDS: f64 = 1.5;
/// Ekolayzerin koruma kısması: taşmayı önlemek için hızlı iner, yavaş kalkar.
const PROTECT_ATTACK_SECONDS: f64 = 0.01;
const PROTECT_RELEASE_SECONDS: f64 = 0.5;

/// Kullanıcının eşitleme ayarı (arayüzden ses iş parçacığına kilitsiz).
#[derive(Debug)]
pub struct LoudnessControl {
    enabled: AtomicBool,
}

impl Default for LoudnessControl {
    fn default() -> Self {
        Self::new(true)
    }
}

impl LoudnessControl {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
        }
    }

    pub fn set(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

const UNKNOWN: u8 = 0;
const MEASURED: u8 = 1;
/// Analiz bitti ama ölçülecek ses yok (sessiz şarkı): kazanç uygulanmaz.
const SILENT: u8 = 2;

/// Bas tepeleri: raf kazançlarındaki artışlar ve alt oktav bandı.
const BASS_VALUES: usize = PEAK_STEPS_DB.len() + 1;

#[derive(Debug)]
struct Slot {
    first_frame: AtomicU64,
    state: AtomicU8,
    lufs: AtomicU64,
    peak: AtomicU64,
    /// Bas tepeleri ölçüldü mü (tam analizden; ses yüksekliğinden sonra gelebilir).
    bass_known: AtomicBool,
    bass: [AtomicU64; BASS_VALUES],
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            first_frame: AtomicU64::new(0),
            state: AtomicU8::new(UNKNOWN),
            lufs: AtomicU64::new(0),
            peak: AtomicU64::new(0),
            bass_known: AtomicBool::new(false),
            bass: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

/// Bir oturumdaki şarkıların başlangıçları ve ses yükseklikleri.
#[derive(Debug, Default)]
pub struct TrackLevels {
    slots: [Slot; SLOTS],
    /// Oturum başından beri eklenen şarkı sayısı.
    count: AtomicUsize,
}

/// Bir şarkının ses iş parçacığının kullandığı seviyesi.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackLevel {
    /// Eşitleme kazancı (dB).
    pub gain_db: f64,
    /// Eşitlemeden sonra tepeye kalan boşluk (dB, ≥ 0): ekolayzer bu kadar yükseltebilir.
    pub headroom_db: f64,
    /// Ölçülmüş bas tepeleri: bas düğmesi tepeyi gerçekte ne kadar yükseltiyor.
    pub bass_peaks: Option<BassPeaks>,
}

impl TrackLevels {
    /// İlk şarkısı oturumun başında başlayan tablo.
    pub fn new() -> Self {
        let levels = Self::default();
        levels.begin_track(0);
        levels
    }

    /// Yeni şarkı ses akışında `first_frame` karesinde başlıyor (çözme iş parçacığı).
    pub fn begin_track(&self, first_frame: u64) {
        let index = self.count.load(Ordering::Acquire);
        let slot = &self.slots[index % SLOTS];
        slot.first_frame.store(first_frame, Ordering::Relaxed);
        slot.state.store(UNKNOWN, Ordering::Relaxed);
        slot.bass_known.store(false, Ordering::Relaxed);
        self.count.store(index + 1, Ordering::Release);
    }

    /// `index`. şarkının bas tepeleri biliniyor mu?
    pub fn has_bass_peaks(&self, index: usize) -> bool {
        self.slot(index)
            .is_some_and(|s| s.bass_known.load(Ordering::Acquire))
    }

    /// `index`. şarkının bas tepelerini yazar.
    pub fn set_bass_peaks(&self, index: usize, peaks: &BassPeaks) {
        let Some(slot) = self.slot(index) else {
            return;
        };
        let values = peaks
            .shelf_rise_db
            .iter()
            .chain(std::iter::once(&peaks.octave_band_db));
        for (cell, value) in slot.bass.iter().zip(values) {
            cell.store(value.to_bits(), Ordering::Relaxed);
        }
        slot.bass_known.store(true, Ordering::Release);
    }

    fn bass_peaks(&self, index: usize) -> Option<BassPeaks> {
        let slot = self.slot(index)?;
        if !slot.bass_known.load(Ordering::Acquire) {
            return None;
        }
        let value = |i: usize| f64::from_bits(slot.bass[i].load(Ordering::Relaxed));
        Some(BassPeaks {
            shelf_rise_db: std::array::from_fn(value),
            octave_band_db: value(BASS_VALUES - 1),
        })
    }

    /// Oturumdaki şarkı sayısı.
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    /// `index`. şarkının ölçümü biliniyor mu?
    pub fn is_known(&self, index: usize) -> bool {
        self.slot(index)
            .is_some_and(|s| s.state.load(Ordering::Acquire) != UNKNOWN)
    }

    /// `index`. şarkının ölçümünü yazar (`None`: sessiz şarkı, ölçülecek ses yok).
    pub fn set_loudness(&self, index: usize, loudness: Option<Loudness>) {
        let Some(slot) = self.slot(index) else {
            return;
        };
        match loudness {
            Some(l) => {
                slot.lufs
                    .store(l.integrated_lufs.to_bits(), Ordering::Relaxed);
                slot.peak
                    .store(l.true_peak_dbtp.to_bits(), Ordering::Relaxed);
                slot.state.store(MEASURED, Ordering::Release);
            }
            None => slot.state.store(SILENT, Ordering::Release),
        }
    }

    /// `index`. şarkının seviyesi; eşitleme kapalıysa kazanç 0, boşluk yalnızca tepeden.
    pub fn level(&self, index: usize, normalize: bool) -> TrackLevel {
        let measured = self
            .slot(index)
            .and_then(|slot| match slot.state.load(Ordering::Acquire) {
                MEASURED => Some(Some(Loudness {
                    integrated_lufs: f64::from_bits(slot.lufs.load(Ordering::Relaxed)),
                    true_peak_dbtp: f64::from_bits(slot.peak.load(Ordering::Relaxed)),
                })),
                SILENT => Some(None),
                _ => None,
            });
        let bass_peaks = self.bass_peaks(index);
        match measured {
            Some(Some(loudness)) => {
                let gain_db = if normalize {
                    loudness.gain_db(TARGET_LUFS)
                } else {
                    0.0
                };
                TrackLevel {
                    gain_db,
                    headroom_db: (-(loudness.true_peak_dbtp + gain_db)).max(0.0),
                    bass_peaks,
                }
            }
            // Sessiz şarkı: ne kazanç ne boşluk.
            Some(None) => TrackLevel {
                gain_db: 0.0,
                headroom_db: 0.0,
                bass_peaks,
            },
            // Henüz ölçülmedi: tipik bir kayıt varsayılır, boşluğa güvenilmez.
            None => TrackLevel {
                gain_db: if normalize {
                    TARGET_LUFS - ASSUMED_LUFS
                } else {
                    0.0
                },
                headroom_db: 0.0,
                bass_peaks,
            },
        }
    }

    /// `index`. şarkıdan sonraki şarkının başladığı kare (yoksa `None`).
    fn next_start(&self, index: usize) -> Option<u64> {
        let count = self.count();
        (index + 1 < count).then(|| {
            self.slots[(index + 1) % SLOTS]
                .first_frame
                .load(Ordering::Relaxed)
        })
    }

    fn slot(&self, index: usize) -> Option<&Slot> {
        let count = self.count();
        (index < count && count - index <= SLOTS).then(|| &self.slots[index % SLOTS])
    }
}

/// Ses iş parçacığında eşitleme kazancını ve ekolayzerin koruma kısmasını uygular.
/// Bellek ayırmaz, beklemez.
#[derive(Debug)]
pub struct Leveler {
    /// Ses akışında sıradaki karenin yeri (oturum başından).
    frame: u64,
    /// Çalan şarkının sırası ve sonraki şarkının başladığı kare.
    track: usize,
    next_start: Option<u64>,
    /// Eşitleme kazancı: şu anki ve hedef (doğrusal).
    gain: f64,
    target: f64,
    /// Ekolayzerin koruma kısması: şu anki ve hedef (doğrusal, ≤ 1).
    protect: f64,
    protect_target: f64,
    switch_coef: f64,
    update_coef: f64,
    attack_coef: f64,
    release_coef: f64,
    /// Şarkı yeni değişti: kazanç kısa sürede hedefe gider.
    switching: bool,
    /// Çalan şarkının boşluğu (dB).
    headroom_db: f64,
    /// Son bildirilen ekolayzer ve bas yükseltmesi (şarkı sınırında yeniden kullanılır).
    eq_db: f64,
    bass: BassBoost,
}

fn coefficient(seconds: f64, sample_rate: f64) -> f64 {
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

impl Leveler {
    /// Oturum başında: ilk şarkının o anki bilinen seviyesiyle (geçişsiz) başlar.
    pub fn new(sample_rate: u32, levels: &TrackLevels, normalize: bool) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let level = levels.level(0, normalize);
        let gain = db_to_linear(level.gain_db);
        Self {
            frame: 0,
            track: 0,
            next_start: levels.next_start(0),
            gain,
            target: gain,
            protect: 1.0,
            protect_target: 1.0,
            switch_coef: coefficient(SWITCH_SECONDS, rate),
            update_coef: coefficient(UPDATE_SECONDS, rate),
            attack_coef: coefficient(PROTECT_ATTACK_SECONDS, rate),
            release_coef: coefficient(PROTECT_RELEASE_SECONDS, rate),
            switching: false,
            headroom_db: level.headroom_db,
            eq_db: 0.0,
            bass: BassBoost::default(),
        }
    }

    /// Her doldurma turunun başında: hedefleri günceller. `eq_db`: ekolayzerin (o anki) en
    /// büyük yükseltmesi; `bass`: bas motorunun yükseltmesi (şarkının ölçülmüş bas
    /// tepeleriyle gerçek artışa çevrilir).
    pub fn begin_block(
        &mut self,
        levels: &TrackLevels,
        normalize: bool,
        eq_db: f64,
        bass: BassBoost,
    ) {
        self.eq_db = eq_db;
        self.bass = bass;
        self.next_start = levels.next_start(self.track);
        let level = levels.level(self.track, normalize);
        self.target = db_to_linear(level.gain_db);
        self.headroom_db = level.headroom_db;
        let boost_db = eq_db + bass.rise_db(level.bass_peaks.as_ref());
        self.protect_target = db_to_linear(-(boost_db - self.headroom_db).max(0.0));
    }

    /// Bir kare için toplam kazanç (eşitleme × koruma); her kare bir kez çağrılır.
    #[inline]
    pub fn next_gain(&mut self, levels: &TrackLevels, normalize: bool) -> Sample {
        if self.next_start.is_some_and(|start| self.frame >= start) {
            // Boşluksuz geçiş: yeni şarkının ilk karesi.
            self.track += 1;
            self.switching = true;
            self.begin_block(levels, normalize, self.eq_db, self.bass);
        }
        self.frame += 1;

        let coef = if self.switching {
            self.switch_coef
        } else {
            self.update_coef
        };
        self.gain = approach(self.gain, self.target, coef);
        if self.gain == self.target {
            self.switching = false;
        }
        let coef = if self.protect_target < self.protect {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.protect = approach(self.protect, self.protect_target, coef);
        self.gain * self.protect
    }

    /// Çalan şarkının boşluğu (dB).
    pub fn headroom_db(&self) -> f64 {
        self.headroom_db
    }
}

/// `value`'yu `target`'a yaklaştırır; çok yaklaşınca tam hedefe oturur (kazanç 1 ise ses
/// bit bit aynen geçsin).
#[inline]
fn approach(value: f64, target: f64, coef: f64) -> f64 {
    let next = value + (target - value) * coef;
    if (next - target).abs() < 1e-9 {
        target
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::bass::BassPeaks;

    const RATE: u32 = 48_000;

    fn loud() -> Loudness {
        Loudness {
            integrated_lufs: -8.0,
            true_peak_dbtp: 0.0,
        }
    }

    fn quiet() -> Loudness {
        Loudness {
            integrated_lufs: -20.0,
            true_peak_dbtp: -12.0,
        }
    }

    fn to_db(gain: f64) -> f64 {
        20.0 * gain.log10()
    }

    fn run(leveler: &mut Leveler, levels: &TrackLevels, frames: usize, boost: f64) -> f64 {
        leveler.begin_block(levels, true, boost, BassBoost::default());
        let mut gain = 0.0;
        for _ in 0..frames {
            gain = leveler.next_gain(levels, true);
        }
        gain
    }

    #[test]
    fn olculmus_sarki_hedefe_getirilir_ve_bosluk_hesaplanir() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud()));
        let level = levels.level(0, true);
        assert_eq!(level.gain_db, -6.0);
        assert_eq!(
            level.headroom_db, 6.0,
            "kısılan şarkının tepesi −6 dBTP'ye iner"
        );
        // Eşitleme kapalıyken kazanç yok; boşluk yalnızca tepeden.
        assert_eq!(
            levels.level(0, false),
            TrackLevel {
                gain_db: 0.0,
                headroom_db: 0.0,
                bass_peaks: None,
            }
        );
        // Ölçülmemiş şarkı: tipik kayıt varsayılır, boşluğa güvenilmez.
        let unknown = TrackLevels::new().level(0, true);
        assert_eq!(unknown.gain_db, -4.0);
        assert_eq!(unknown.headroom_db, 0.0);
    }

    #[test]
    fn olcum_baslangicta_biliniyorsa_gecis_olmadan_baslar() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud()));
        let mut leveler = Leveler::new(RATE, &levels, true);
        let first = leveler.next_gain(&levels, true);
        assert!((to_db(first) + 6.0).abs() < 1e-9);
    }

    #[test]
    fn olcum_sonradan_gelince_yavasca_duzelir() {
        let levels = TrackLevels::new();
        let mut leveler = Leveler::new(RATE, &levels, true);
        assert!((to_db(run(&mut leveler, &levels, 10, 0.0)) + 4.0).abs() < 1e-6);
        levels.set_loudness(0, Some(quiet())); // +6 dB'ye çıkmalı (tepe izin veriyor)
        let after_100ms = to_db(run(&mut leveler, &levels, RATE as usize / 10, 0.0));
        assert!(
            after_100ms > -4.0 && after_100ms < 0.0,
            "ani değil: {after_100ms}"
        );
        let settled = to_db(run(&mut leveler, &levels, RATE as usize * 15, 0.0));
        assert!((settled - 6.0).abs() < 0.01, "{settled}");
    }

    #[test]
    fn bosluksuz_geciste_kazanc_tam_sinirda_degisir() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud()));
        let mut leveler = Leveler::new(RATE, &levels, true);
        // 1. şarkı 1000. karede biter; 2. şarkı (sessiz kayıt) o karede başlar.
        levels.begin_track(1000);
        levels.set_loudness(1, Some(quiet()));
        leveler.begin_block(&levels, true, 0.0, BassBoost::default());
        let mut gains = Vec::new();
        for _ in 0..1000 + RATE as usize {
            gains.push(to_db(leveler.next_gain(&levels, true)));
        }
        assert!(
            gains[..1000].iter().all(|&g| (g + 6.0).abs() < 1e-9),
            "sınırdan önce −6"
        );
        assert!(gains[1000] > -6.0, "sınırda değişmeye başlar");
        assert!(gains[1000] < -5.0, "tık olmasın diye kısa bir geçişle");
        assert!((gains.last().unwrap() - 6.0).abs() < 0.01, "sonra +6");
    }

    #[test]
    fn ekolayzer_ancak_bosluk_yetmezse_kisilir() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud())); // −6 dB eşitleme → 6 dB boşluk
        let mut leveler = Leveler::new(RATE, &levels, true);
        // +6 dB bas: boşluk yetiyor, koruma kısması yok.
        let g = to_db(run(&mut leveler, &levels, RATE as usize, 6.0));
        assert!((g + 6.0).abs() < 1e-6, "{g}");
        // +10 dB: 4 dB yetmiyor, hızla (10 ms) kısılır.
        let g = to_db(run(&mut leveler, &levels, RATE as usize / 10, 10.0));
        assert!((g + 10.0).abs() < 0.01, "{g}");
        // Yükseltme azalınca koruma yavaşça kalkar.
        let g = to_db(run(&mut leveler, &levels, RATE as usize / 100, 0.0));
        assert!(g < -9.0, "hemen kalkmaz: {g}");
        let g = to_db(run(&mut leveler, &levels, RATE as usize * 10, 0.0));
        assert!((g + 6.0).abs() < 1e-6, "{g}");
    }

    #[test]
    fn olculen_bas_tepeleri_korumayi_gereksiz_kismaktan_kurtarir() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud())); // −6 dB eşitleme → 6 dB boşluk
        let bass = BassBoost {
            shelf_db: 12.0,
            ..BassBoost::default()
        };
        let mut leveler = Leveler::new(RATE, &levels, true);
        // Ölçüm yok: en kötü durum, rafın tamamı (12 − 6 = 6 dB kısılır).
        leveler.begin_block(&levels, true, 0.0, bass);
        let mut gain = 0.0;
        for _ in 0..RATE {
            gain = leveler.next_gain(&levels, true);
        }
        assert!((to_db(gain) + 12.0).abs() < 0.01, "{}", to_db(gain));
        // Ölçüldü: bu şarkıda +12 dB bas tepeyi yalnızca 2,5 dB yükseltiyor; boşluk yetiyor.
        levels.set_bass_peaks(
            0,
            &BassPeaks {
                shelf_rise_db: [0.3, 0.8, 1.5, 2.5, 4.0, 6.0, 8.0, 10.5, 13.0],
                octave_band_db: -9.0,
            },
        );
        assert!(levels.has_bass_peaks(0));
        leveler.begin_block(&levels, true, 0.0, bass);
        for _ in 0..RATE * 10 {
            gain = leveler.next_gain(&levels, true);
        }
        assert!(
            (to_db(gain) + 6.0).abs() < 0.01,
            "yalnızca eşitleme: {}",
            to_db(gain)
        );
        // Yeni şarkı başlayınca ölçümü yoktur.
        levels.begin_track(1_000_000);
        assert!(!levels.has_bass_peaks(1));
    }

    #[test]
    fn esitleme_kapali_ve_ekolayzer_duzken_kazanc_tam_bir() {
        let levels = TrackLevels::new();
        levels.set_loudness(0, Some(loud()));
        let mut leveler = Leveler::new(RATE, &levels, false);
        leveler.begin_block(&levels, false, 0.0, BassBoost::default());
        for _ in 0..1000 {
            assert_eq!(leveler.next_gain(&levels, false), 1.0);
        }
    }

    #[test]
    fn eski_sarkilar_tablodan_duser() {
        let levels = TrackLevels::new();
        for i in 1..20u64 {
            levels.begin_track(i * 100);
        }
        assert_eq!(levels.count(), 20);
        assert!(!levels.is_known(0), "çok eskisi artık tutulmuyor");
        levels.set_loudness(0, Some(loud())); // yok sayılır, yeni şarkıyı bozmaz
        assert!(!levels.is_known(16));
        levels.set_loudness(19, Some(loud()));
        assert!(levels.is_known(19));
    }
}
