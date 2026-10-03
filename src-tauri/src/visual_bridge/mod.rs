//! Görsel köprüsü.
//!
//! Ses motorunun çalma zamanını ve analiz sonuçlarını arayüzdeki görsellere
//! taşır. Şimdilik spektrum ve kanal seviyeleri; Faz 2'de şarkı haritası, gecikme
//! telafisi ve kalibrasyon (hedef: ±20 ms senkron) burada yapılacak.

use serde::Serialize;

use crate::audio::player::VisualData;

/// Arayüzün her ekran karesinde istediği görsel veri.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualFrame {
    /// Verinin ait olduğu çalma konumu (saniye).
    pub position_secs: f64,
    /// Logaritmik aralıklı frekans bantları (bastan tize), 0..1.
    pub bands: Vec<f32>,
    /// Sol/sağ etkin (RMS) seviye, dBFS. Sessizlik −60.
    pub rms_db: [f32; 2],
    /// Sol/sağ tepe seviye, dBFS. Sessizlik −60.
    pub peak_db: [f32; 2],
    /// 0 VU'ya denk gelen seviye (dBFS): şarkının yüksek bölümlerine göre.
    /// Analiz bitene kadar `null`.
    pub vu_reference_db: Option<f32>,
}

impl From<VisualData> for VisualFrame {
    fn from(data: VisualData) -> Self {
        Self {
            position_secs: data.seconds,
            bands: data.bands.to_vec(),
            rms_db: data.levels.rms_db,
            peak_db: data.levels.peak_db,
            vu_reference_db: data.vu_reference_db,
        }
    }
}

/// Görsel köprüsünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Spektrum ve kanal seviyeleri aktarılıyor; şarkı haritası Faz 2'de.
    Spectrum,
}

impl BridgeStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "spektrum ve VU aktif",
        }
    }
}

/// Görsel köprüsünün şu anki durumu.
pub fn status() -> BridgeStatus {
    BridgeStatus::Spectrum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::levels::ChannelLevels;

    #[test]
    fn arayuze_camel_case_gonderilir() {
        let frame = VisualFrame::from(VisualData {
            seconds: 1.5,
            bands: [0.5; crate::analysis::spectrogram::BANDS],
            levels: ChannelLevels {
                rms_db: [-12.0, -14.0],
                peak_db: [-3.0, -4.0],
            },
            vu_reference_db: None,
        });
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["positionSecs"], 1.5);
        assert_eq!(json["rmsDb"][1], -14.0);
        assert_eq!(json["peakDb"][0], -3.0);
        assert!(json["vuReferenceDb"].is_null());
        assert_eq!(json["bands"].as_array().unwrap().len(), 32);
    }
}
