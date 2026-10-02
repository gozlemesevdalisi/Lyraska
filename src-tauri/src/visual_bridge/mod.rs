//! Görsel köprüsü.
//!
//! Ses motorunun çalma zamanını ve şarkı haritasını arayüzdeki WebGL2
//! görsellerine taşır. Gecikme telafisi ve kalibrasyon (hedef: ±20 ms
//! senkron) burada yapılacak.

/// Görsel köprüsünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Henüz yazılmadı (Faz 0).
    NotImplemented,
}

impl BridgeStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotImplemented => "Faz 1'de geliyor",
        }
    }
}

/// Görsel köprüsünün şu anki durumu.
pub fn status() -> BridgeStatus {
    BridgeStatus::NotImplemented
}
