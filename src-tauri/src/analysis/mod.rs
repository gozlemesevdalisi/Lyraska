//! Şarkı haritası analizi.
//!
//! Faz 2'de burada şunlar olacak: beat ve ölçü takibi, bölüm sınırları,
//! drop tespiti ve enerji eğrisi. Analiz şarkı çalmadan önce yapılır ve
//! sonuç SQLite'ta saklanır; böylece görseller şarkıyı "önceden bilir".

/// Analiz modülünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    /// Henüz yazılmadı (Faz 0).
    NotImplemented,
}

impl AnalysisStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotImplemented => "Faz 2'de geliyor",
        }
    }
}

/// Analiz modülünün şu anki durumu.
pub fn status() -> AnalysisStatus {
    AnalysisStatus::NotImplemented
}
