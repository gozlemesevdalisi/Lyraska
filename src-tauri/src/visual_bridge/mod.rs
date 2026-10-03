//! Görsel köprüsü.
//!
//! Ses motorunun çalma zamanını ve analiz sonuçlarını arayüzdeki görsellere
//! taşır. Şimdilik spektrum karesi; Faz 2'de şarkı haritası, gecikme telafisi
//! ve kalibrasyon (hedef: ±20 ms senkron) burada yapılacak.

use serde::Serialize;

/// Arayüzün her ekran karesinde istediği görsel veri.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualFrame {
    /// Verinin ait olduğu çalma konumu (saniye).
    pub position_secs: f64,
    /// Logaritmik aralıklı frekans bantları (bastan tize), 0..1.
    pub bands: Vec<f32>,
}

/// Görsel köprüsünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Spektrum aktarılıyor; şarkı haritası Faz 2'de.
    Spectrum,
}

impl BridgeStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "spektrum aktif",
        }
    }
}

/// Görsel köprüsünün şu anki durumu.
pub fn status() -> BridgeStatus {
    BridgeStatus::Spectrum
}
