//! İnsan işaretleri: kullanıcının şarkı çalarken tuşla işaretlediği beat ve drop anları.
//!
//! Analizin doğruluğunu gerçek müzikte ölçmek için kullanılır (hedef: beat
//! F-ölçüsü ≥ 0,80). Her şarkı için uygulama veri klasöründeki `isaretler/`
//! klasörüne bir JSON dosyası yazılır. Dosyada **ses yoktur**: yalnızca zamanlar,
//! şarkının adı/sanatçısı/süresi ve (kullanıcının kendi bilgisayarında yeniden
//! değerlendirme için) dosya yolu. Ses dosyaları telif nedeniyle hiçbir zaman
//! depoya girmez.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::evaluate::BeatEvaluation;
use crate::audio::decode::TrackInfo;

/// Dosya biçiminin sürümü (alanlar değişirse artar).
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub format: u32,
    pub app_version: String,
    pub saved_at: String,
    pub track: AnnotatedTrack,
    /// İşaretlenen vuruşlar (saniye, artan).
    pub beats: Vec<f64>,
    /// İşaretlenen droplar (saniye, artan).
    pub drops: Vec<f64>,
    /// Son "doğruluğu ölç" sonucu.
    #[serde(default)]
    pub evaluation: Option<BeatEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedTrack {
    /// Bu bilgisayardaki yol (yeniden değerlendirme için; başka yere gönderilmez).
    pub path: PathBuf,
    pub file_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_secs: Option<f64>,
    pub size_bytes: Option<u64>,
}

impl AnnotatedTrack {
    pub fn from_info(info: &TrackInfo) -> Self {
        Self {
            path: info.path.clone(),
            file_name: info.file_name.clone(),
            title: info.title.clone(),
            artist: info.artist.clone(),
            duration_secs: info.duration_secs,
            size_bytes: std::fs::metadata(&info.path).ok().map(|m| m.len()),
        }
    }
}

impl Annotation {
    pub fn new(track: AnnotatedTrack, beats: Vec<f64>, drops: Vec<f64>) -> Self {
        Self {
            format: FORMAT,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            saved_at: crate::diagnostics::timestamp(),
            track,
            beats: clean_times(beats),
            drops: clean_times(drops),
            evaluation: None,
        }
    }
}

/// Geçersiz değerleri atar, sıralar.
fn clean_times(mut times: Vec<f64>) -> Vec<f64> {
    times.retain(|t| t.is_finite() && *t >= 0.0);
    times.sort_by(f64::total_cmp);
    times
}

/// İşaret dosyalarının klasörü.
pub struct AnnotationStore {
    dir: Option<PathBuf>,
}

impl AnnotationStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir: Some(dir) }
    }

    /// Klasörü bilinmeyen depo (uygulama veri klasörü bulunamadıysa).
    pub fn unavailable() -> Self {
        Self { dir: None }
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Şarkının işaret dosyası (şarkı yolu aynı kaldıkça aynı dosya).
    pub fn file_for(&self, track_path: &Path, label: &str) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(file_name(track_path, label)))
    }

    /// Yazar; yazılan dosyanın yolunu döndürür. Önce geçici dosyaya yazılır.
    /// Şarkının etiketi değiştiyse eski adlı dosyası kaldırılır (tek kopya kalır).
    pub fn save(&self, annotation: &Annotation) -> std::io::Result<PathBuf> {
        let track_path = &annotation.track.path;
        let path = self
            .file_for(track_path, &label(&annotation.track))
            .ok_or_else(|| std::io::Error::other("uygulama veri klasörü bulunamadı"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(annotation).map_err(std::io::Error::other)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &path)?;
        for old in self.files_of(track_path) {
            if old != path {
                let _ = std::fs::remove_file(old);
            }
        }
        Ok(path)
    }

    /// Şarkının kayıtlı işaretleri (yoksa ya da okunamazsa `None`). Dosya adı etiketten
    /// gelir; etiket değiştiyse dosya, adının sonundaki şarkı yolu özetinden bulunur.
    pub fn load(&self, track: &AnnotatedTrack) -> Option<Annotation> {
        let current = self.file_for(&track.path, &label(track))?;
        read(&current)
            .filter(|a| a.track.path == track.path)
            .or_else(|| {
                self.files_of(&track.path)
                    .iter()
                    .filter_map(|p| read(p))
                    .max_by(|a, b| a.saved_at.cmp(&b.saved_at))
            })
    }

    /// Bu şarkıya ait bütün işaret dosyaları: adı şarkı yolunun özetiyle biten ve
    /// içindeki yol şarkınınki olan (özet çakışırsa başka şarkınınki karışmaz).
    fn files_of(&self, track_path: &Path) -> Vec<PathBuf> {
        let Some(entries) = self.dir.as_ref().and_then(|d| std::fs::read_dir(d).ok()) else {
            return Vec::new();
        };
        let suffix = path_suffix(track_path);
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(&suffix))
            })
            .filter(|p| read(p).is_some_and(|a| a.track.path == track_path))
            .collect()
    }
}

fn read(path: &Path) -> Option<Annotation> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Dosya adında gösterilecek ad: "Sanatçı - Başlık" ya da dosya adı.
pub fn label(track: &AnnotatedTrack) -> String {
    match (&track.artist, &track.title) {
        (Some(artist), Some(title)) => format!("{artist} - {title}"),
        (None, Some(title)) => title.clone(),
        _ => track.file_name.clone(),
    }
}

/// Windows'ta geçerli, okunur ve şarkı yoluna göre benzersiz dosya adı.
fn file_name(track_path: &Path, label: &str) -> String {
    let safe: String = label
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let safe = safe.trim().trim_end_matches('.').trim();
    let safe = if safe.is_empty() { "sarki" } else { safe };
    format!("{safe}{}", path_suffix(track_path))
}

/// Dosya adının şarkı yolundan gelen sonu: ".1a2b3c4d.json".
fn path_suffix(track_path: &Path) -> String {
    format!(
        ".{:08x}.json",
        fnv1a(track_path.to_string_lossy().as_bytes()) as u32
    )
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::temp_path;

    fn track(path: &str) -> AnnotatedTrack {
        AnnotatedTrack {
            path: PathBuf::from(path),
            file_name: "01 Gece".into(),
            title: Some("Gece: Otoyol?".into()),
            artist: Some("Lyra".into()),
            duration_secs: Some(200.0),
            size_bytes: None,
        }
    }

    #[test]
    fn dosya_adi_gecerli_ve_benzersiz() {
        let a = file_name(Path::new("C:\\Müzik\\a.mp3"), "Lyra - Gece: Otoyol?");
        assert!(a.starts_with("Lyra - Gece_ Otoyol_."), "{a}");
        assert!(a.ends_with(".json"));
        let b = file_name(Path::new("C:\\Müzik\\b.mp3"), "Lyra - Gece: Otoyol?");
        assert_ne!(a, b, "aynı adlı farklı dosyalar karışmaz");
        assert!(file_name(Path::new("x"), "  ...  ").starts_with("sarki."));
        assert_eq!(label(&track("x")), "Lyra - Gece: Otoyol?");
    }

    #[test]
    fn kaydeder_ve_geri_okur() {
        let dir = temp_path("isaretler").with_extension("");
        let store = AnnotationStore::new(dir.clone());
        let annotation = Annotation::new(
            track("C:\\Müzik\\gece.flac"),
            vec![2.0, 1.5, f64::NAN, -1.0, 2.5],
            vec![30.0],
        );
        assert_eq!(annotation.beats, vec![1.5, 2.0, 2.5], "sıralı ve temiz");
        let path = store.save(&annotation).unwrap();
        assert!(path.starts_with(&dir));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"beats\"") && text.contains("\"appVersion\""),
            "{text}"
        );
        assert_eq!(store.load(&annotation.track), Some(annotation.clone()));
        assert_eq!(store.load(&track("C:\\Müzik\\yok.flac")), None);
        assert!(AnnotationStore::unavailable().save(&annotation).is_err());
    }

    #[test]
    fn etiket_degisse_de_isaretler_bulunur() {
        // Dosya adı etiketten gelir; etiket değişince (yeniden etiketleme, ID3v1
        // etiketlerinin okunmaya başlanması) kayıtlı işaretler kaybolmamalı.
        let dir = temp_path("isaretler-etiket").with_extension("");
        let store = AnnotationStore::new(dir.clone());
        let before = track("C:\\Müzik\\gece.flac");
        store
            .save(&Annotation::new(before.clone(), vec![1.0, 1.5], vec![]))
            .unwrap();

        let after = AnnotatedTrack {
            title: None,
            artist: None,
            ..before
        };
        let loaded = store.load(&after).expect("etiket değişince de bulunmalı");
        assert_eq!(loaded.beats, vec![1.0, 1.5]);

        // Yeni kayıt yeni ada yazılır, eski dosya kalmaz (iki kopya karışmasın).
        store
            .save(&Annotation::new(after.clone(), vec![1.0, 1.5, 2.0], vec![]))
            .unwrap();
        let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(store.load(&after).unwrap().beats.len(), 3);

        // Aynı özetli başka şarkının dosyası karıştırılmaz.
        let other = AnnotatedTrack {
            path: PathBuf::from("C:\\Müzik\\baska.flac"),
            ..after
        };
        assert_eq!(store.load(&other), None);
    }
}
