//! Görsel köprüsü.
//!
//! Ses motorunun çalma zamanını ve analiz sonuçlarını arayüzdeki görsellere
//! taşır: spektrum, kanal seviyeleri, vuruşlar, şarkı haritası ve Görsel
//! Yönetmen'in notu.
//!
//! **Gecikme telafisi** (hedef: ±20 ms senkron). Oynatıcı "şu an duyulan" anı ses
//! aygıtının tamponunu ve taşma korumasını hesaba katarak bilir. İki gecikme kalır:
//!
//! - **Ekran:** görsel veri istendikten sonra ekranda ortalama ~1,5 kare (60 Hz'de
//!   ~25 ms) sonra görünür. Görseller bu kadar ileriden okunur ([`DISPLAY_LEAD_SECONDS`]).
//! - **Ses aygıtı:** bazı aygıtlar (özellikle Bluetooth kulaklıklar) sesi Windows'un
//!   bildirdiğinden 100–300 ms geç çalar; yazılım bunu bilemez. Kullanıcı ayarlar
//!   ya da [`calibration`] tıklama kaydıyla ölçer; görseller o kadar geriden okunur.

pub mod calibration;

use serde::Serialize;

use crate::audio::player::VisualData;

/// Görsel verinin istenmesiyle ekranda görünmesi arasındaki ortalama süre (saniye).
pub const DISPLAY_LEAD_SECONDS: f64 = 0.025;
/// Ses gecikmesi ayarının sınırları (milisaniye). Eksi değer: ses beklenenden önce duyuluyor.
pub const MIN_AUDIO_DELAY_MS: i32 = -100;
pub const MAX_AUDIO_DELAY_MS: i32 = 400;

/// Ses gecikmesi ayarını sınırlar içine alır.
pub fn clamp_audio_delay_ms(ms: i32) -> i32 {
    ms.clamp(MIN_AUDIO_DELAY_MS, MAX_AUDIO_DELAY_MS)
}

/// Görsellerin okunacağı şarkı anı: duyulan an + ekran gecikmesi − ses aygıtının
/// ek gecikmesi. Şarkının başından önceye gitmez.
pub fn visual_time(heard_secs: f64, audio_delay_ms: i32) -> f64 {
    let delay = f64::from(clamp_audio_delay_ms(audio_delay_ms)) / 1000.0;
    (heard_secs + DISPLAY_LEAD_SECONDS - delay).max(0.0)
}

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
    /// Tempo ve vuruş konumu; analiz bitene kadar ya da ritim yoksa `null`.
    pub beat: Option<BeatFrame>,
    /// Şarkının o anki enerjisi (0..1, şarkıya göre); analiz bitene kadar `null`.
    pub energy: Option<f32>,
    /// O anki bölümün sırası; analiz bitene kadar `null`.
    pub section: Option<usize>,
    /// Görsel Yönetmen'in o anki notu (atmosfer, ritim, doku); analiz bitene kadar `null`.
    pub director: Option<crate::director::DirectorFrame>,
}

/// O anın vuruş ızgarasındaki yeri.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatFrame {
    /// Şarkının temposu (vuruş/dakika).
    pub bpm: f64,
    /// Son vuruşun sırası (0'dan başlar).
    pub index: usize,
    /// Son vuruştan bu yana geçen süre, vuruş aralığına oranla (0..1).
    pub phase: f64,
    /// Vuruşun ölçüdeki yeri (1 = ölçü başı); yapı analizi bitene kadar `null`.
    pub bar_beat: Option<usize>,
}

impl From<VisualData> for VisualFrame {
    fn from(data: VisualData) -> Self {
        Self {
            position_secs: data.seconds,
            bands: data.bands.to_vec(),
            rms_db: data.levels.rms_db,
            peak_db: data.levels.peak_db,
            vu_reference_db: data.vu_reference_db,
            beat: data.beat.map(|(bpm, position)| BeatFrame {
                bpm,
                index: position.index,
                phase: position.phase,
                bar_beat: data.structure.and_then(|s| s.bar_beat),
            }),
            energy: data.structure.and_then(|s| s.energy),
            section: data.structure.and_then(|s| s.section),
            director: data.director,
        }
    }
}

/// Görsel köprüsünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Spektrum, şarkı haritası ve Yönetmen aktarılıyor; gecikme telafili.
    Synced,
}

impl BridgeStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::Synced => "spektrum, şarkı haritası ve Görsel Yönetmen; gecikme telafili",
        }
    }
}

/// Görsel köprüsünün şu anki durumu.
pub fn status() -> BridgeStatus {
    BridgeStatus::Synced
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::levels::ChannelLevels;

    #[test]
    fn gorsel_zamani_ekran_ve_ses_gecikmesini_telafi_eder() {
        // Gecikme yok: yalnızca ekran gecikmesi kadar ileri.
        assert!((visual_time(10.0, 0) - (10.0 + DISPLAY_LEAD_SECONDS)).abs() < 1e-12);
        // Bluetooth kulaklık 200 ms geç çalıyor: görseller o kadar geriden.
        assert!((visual_time(10.0, 200) - (10.0 + DISPLAY_LEAD_SECONDS - 0.2)).abs() < 1e-12);
        // Ses erken: görseller ileriden.
        assert!((visual_time(10.0, -50) - (10.0 + DISPLAY_LEAD_SECONDS + 0.05)).abs() < 1e-12);
        // Sınırlar ve şarkı başı.
        assert!((visual_time(10.0, 5_000) - visual_time(10.0, MAX_AUDIO_DELAY_MS)).abs() < 1e-12);
        assert_eq!(visual_time(0.1, 400), 0.0);
        assert_eq!(clamp_audio_delay_ms(-1_000), MIN_AUDIO_DELAY_MS);
    }

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
            beat: Some((
                128.0,
                crate::analysis::beats::BeatPosition {
                    index: 7,
                    phase: 0.25,
                },
            )),
            structure: Some(crate::audio::player::StructureNow {
                bar_beat: Some(4),
                energy: Some(0.75),
                section: Some(2),
            }),
            director: None,
        });
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["positionSecs"], 1.5);
        assert_eq!(json["rmsDb"][1], -14.0);
        assert_eq!(json["peakDb"][0], -3.0);
        assert!(json["vuReferenceDb"].is_null());
        assert_eq!(json["beat"]["bpm"], 128.0);
        assert_eq!(json["beat"]["index"], 7);
        assert_eq!(json["beat"]["phase"], 0.25);
        assert_eq!(json["beat"]["barBeat"], 4);
        assert_eq!(json["energy"], 0.75);
        assert_eq!(json["section"], 2);
        assert!(json["director"].is_null());
        assert_eq!(json["bands"].as_array().unwrap().len(), 32);
    }
}
