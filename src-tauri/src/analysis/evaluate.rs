//! Analizin doğruluğunu insan işaretlerine göre ölçme.
//!
//! Beat takibinde yaygın ölçüt **F-ölçüsü**dür (MIREX): her işaretli vuruşa en
//! fazla bir bulunan vuruş, ±70 ms içinde eşleşir; isabet (precision) ve kapsama
//! (recall) birleştirilir. Hedef (yol haritası): F ≥ 0,80.
//!
//! İnsan tuşa vuruşu biraz geç (ya da erken) basar. Bu sabit kayma ayrıca ölçülür
//! ve kaydırılmış F-ölçüsü de verilir: böylece analizin hatası ile parmağın
//! gecikmesi birbirinden ayrılır.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::spectrogram::{analyze, Spectrogram};
use crate::audio::decode::Decoder;
use crate::audio::AudioError;

/// Eşleşme toleransı (saniye).
pub const BEAT_TOLERANCE: f64 = 0.07;

/// Bir şarkının beat değerlendirmesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatEvaluation {
    /// İşaretlere doğrudan göre F-ölçüsü (0..1).
    pub f_measure: f64,
    /// İşaretlerin bulunan vuruşlara göre ortanca kayması (ms; artı: işaret geç).
    pub tap_offset_ms: f64,
    /// Bu kayma düzeltildikten sonraki F-ölçüsü.
    pub f_measure_aligned: f64,
    pub detected_bpm: Option<f64>,
    /// İşaretlerden çıkan tempo (ardışık vuruş aralıklarının ortancası).
    pub marked_bpm: Option<f64>,
    pub detected_count: usize,
    pub marked_count: usize,
}

/// Bulunan vuruşları (`detected`) işaretlere (`marked`) göre değerlendirir.
pub fn evaluate_beats(
    detected: &[f64],
    detected_bpm: Option<f64>,
    marked: &[f64],
) -> BeatEvaluation {
    let offset = tap_offset(detected, marked);
    let aligned: Vec<f64> = marked.iter().map(|t| t - offset).collect();
    BeatEvaluation {
        f_measure: f_measure(detected, marked, BEAT_TOLERANCE),
        tap_offset_ms: (offset * 1000.0 * 10.0).round() / 10.0,
        f_measure_aligned: f_measure(detected, &aligned, BEAT_TOLERANCE),
        detected_bpm,
        marked_bpm: interval_bpm(marked),
        detected_count: detected.len(),
        marked_count: marked.len(),
    }
}

/// Şarkıyı baştan analiz eder ve bulunan vuruşları işaretlere göre değerlendirir.
/// Birkaç saniye sürer; arayüz iş parçacığında çağrılmaz.
pub fn evaluate_file(path: &Path, marked: &[f64]) -> Result<BeatEvaluation, AudioError> {
    let spectrogram = Spectrogram::new();
    analyze(Decoder::open(path)?, &spectrogram);
    let grid = spectrogram.beat_grid();
    let detected = grid.as_ref().map_or(&[][..], |g| &g.beats[..]);
    Ok(evaluate_beats(
        detected,
        grid.as_ref().map(|g| g.bpm),
        marked,
    ))
}

/// F-ölçüsü: her gerçek vuruşa en fazla bir tahmin, `tolerance` saniye içinde.
/// İki liste de artan sırada olmalıdır.
pub fn f_measure(estimated: &[f64], truth: &[f64], tolerance: f64) -> f64 {
    if estimated.is_empty() || truth.is_empty() {
        return 0.0;
    }
    // Sıralı listelerde açgözlü eşleştirme: her gerçek vuruşa, henüz kullanılmamış
    // en yakın tahmin.
    let mut used = vec![false; estimated.len()];
    let mut start = 0usize;
    let mut hits = 0usize;
    for &t in truth {
        while start < estimated.len() && estimated[start] < t - tolerance {
            start += 1;
        }
        let best = (start..estimated.len())
            .take_while(|&i| estimated[i] <= t + tolerance)
            .filter(|&i| !used[i])
            .min_by(|&a, &b| {
                (estimated[a] - t)
                    .abs()
                    .total_cmp(&(estimated[b] - t).abs())
            });
        if let Some(i) = best {
            used[i] = true;
            hits += 1;
        }
    }
    if hits == 0 {
        return 0.0;
    }
    let precision = hits as f64 / estimated.len() as f64;
    let recall = hits as f64 / truth.len() as f64;
    2.0 * precision * recall / (precision + recall)
}

/// İşaretlerin en yakın bulunan vuruşa göre ortanca kayması (saniye).
/// 100 ms'den uzaktaki işaretler (yanlış vuruş) hesaba katılmaz.
pub fn tap_offset(detected: &[f64], marked: &[f64]) -> f64 {
    let mut diffs: Vec<f64> = marked
        .iter()
        .filter_map(|&m| {
            let i = detected.partition_point(|&d| d < m);
            [i.checked_sub(1), Some(i)]
                .into_iter()
                .flatten()
                .filter_map(|j| detected.get(j))
                .map(|&d| m - d)
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        })
        .filter(|d| d.abs() <= 0.1)
        .collect();
    if diffs.is_empty() {
        return 0.0;
    }
    diffs.sort_by(f64::total_cmp);
    diffs[diffs.len() / 2]
}

/// Ardışık aralıkların ortancasından tempo; en az dört işaret gerekir.
pub fn interval_bpm(times: &[f64]) -> Option<f64> {
    let mut intervals: Vec<f64> = times
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0.2 && d < 2.0)
        .collect();
    if intervals.len() < 3 {
        return None;
    }
    intervals.sort_by(f64::total_cmp);
    Some((600.0 / intervals[intervals.len() / 2]).round() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(start: f64, step: f64, n: usize) -> Vec<f64> {
        (0..n).map(|i| start + step * i as f64).collect()
    }

    #[test]
    fn f_olcusu_tam_eslesme_ve_kaymalar() {
        let truth = grid(1.0, 0.5, 40);
        assert_eq!(f_measure(&truth, &truth, BEAT_TOLERANCE), 1.0);
        // 60 ms kayma tolerans içinde, 80 ms dışında.
        let late: Vec<f64> = truth.iter().map(|t| t + 0.06).collect();
        assert_eq!(f_measure(&late, &truth, BEAT_TOLERANCE), 1.0);
        let too_late: Vec<f64> = truth.iter().map(|t| t + 0.08).collect();
        assert_eq!(f_measure(&too_late, &truth, BEAT_TOLERANCE), 0.0);
        // Çift tempo: isabet yarı, kapsama tam → F = 2/3.
        let double = grid(1.0, 0.25, 80);
        assert!((f_measure(&double, &truth, BEAT_TOLERANCE) - 2.0 / 3.0).abs() < 0.02);
        assert_eq!(f_measure(&[], &truth, BEAT_TOLERANCE), 0.0);
    }

    #[test]
    fn parmak_gecikmesi_ayrilir() {
        let detected = grid(0.5, 0.5, 60);
        // İşaretler 90 ms geç ve biraz dağınık: doğrudan F düşük, kaydırınca yüksek.
        let marked: Vec<f64> = detected
            .iter()
            .enumerate()
            .map(|(i, t)| t + 0.09 + if i % 2 == 0 { 0.01 } else { -0.01 })
            .collect();
        let e = evaluate_beats(&detected, Some(120.0), &marked);
        assert!(e.f_measure < 0.6, "{}", e.f_measure);
        assert!(
            (e.tap_offset_ms - 90.0).abs() <= 10.0,
            "{}",
            e.tap_offset_ms
        );
        assert!(e.f_measure_aligned > 0.99, "{}", e.f_measure_aligned);
        assert!(
            (e.marked_bpm.unwrap() - 120.0).abs() < 6.0,
            "{:?}",
            e.marked_bpm
        );
        assert_eq!(e.detected_count, 60);
    }

    #[test]
    fn dosyadan_uctan_uca_degerlendirme() {
        use crate::analysis::beats::tests::drum_track;
        use crate::audio::test_util::{temp_path, write_wav};
        let rate = 22_050;
        let (samples, truth) = drum_track(120.0, 0.8, 20.0, rate, 9);
        let path = temp_path("degerlendirme.wav");
        write_wav(&path, rate, 1, samples.len(), |f, _| 0.8 * samples[f]);
        // İnsan gibi: 40 ms geç basılmış işaretler.
        let marked: Vec<f64> = truth.iter().map(|t| t + 0.04).collect();
        let e = evaluate_file(&path, &marked).unwrap();
        assert!(e.f_measure > 0.95, "{e:?}");
        assert!((e.tap_offset_ms - 40.0).abs() < 8.0, "{e:?}");
        assert!(e.f_measure_aligned > 0.95, "{e:?}");
        assert!((e.detected_bpm.unwrap() - 120.0).abs() < 0.5, "{e:?}");
        assert!(evaluate_file(&path.with_extension("yok"), &marked).is_err());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn tempo_ve_bos_durumlar() {
        assert_eq!(interval_bpm(&grid(0.0, 0.4687, 10)), Some(128.0));
        assert_eq!(interval_bpm(&[1.0, 1.5]), None);
        assert_eq!(tap_offset(&[], &[1.0]), 0.0);
    }
}
