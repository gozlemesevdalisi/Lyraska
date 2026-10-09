//! Ses dosyası çözme (symphonia).
//!
//! Desteklenen biçimler: MP3, FLAC, WAV, AIFF, OGG Vorbis, AAC ve ALAC (M4A/MP4), CAF, MKV/WebM.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::Serialize;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::well_known::METADATA_ID_ID3V1;
use symphonia::core::meta::{MetadataOptions, StandardTag, StandardVisualKey, Visual};
use symphonia::core::units::{Time, TimeBase};

use super::gapless::{self, Trim};
use super::{AudioError, Sample};

/// Kodlayıcı dolgusu çözücü tarafından atılmayan, MP4 tabanlı uzantılar.
const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "mp4"];

/// "Dosya aç" penceresinde gösterilen uzantılar.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "wave", "aif", "aiff", "aifc", "ogg", "oga", "m4a", "m4b", "mp4", "aac",
    "caf", "mka", "webm",
];

/// Açılan şarkının bilgileri.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct TrackInfo {
    pub path: PathBuf,
    /// Dosya adı (uzantısız); etiket yoksa başlık olarak kullanılır.
    pub file_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    /// Kodek adı (ör. "flac", "mp3").
    pub codec: String,
    pub sample_rate: u32,
    pub channels: usize,
    /// Süre (saniye); dosya bildirmiyorsa `None`.
    pub duration_secs: Option<f64>,
}

/// Bir ses dosyasını parça parça 64-bit örneklere çözer.
pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    info: TrackInfo,
    /// Son çözülen paketin örnekleri (kanallar iç içe: L, R, L, R...).
    buffer: Vec<Sample>,
    /// Sarmadan sonra bu saniyeden önceki örnekler atılır (örnek hassasiyetinde sarma).
    /// Çözülen akışın zamanıdır (kodlayıcı dolgusu dahil).
    skip_until: Option<f64>,
    /// Boşluksuz çalma: baştaki ve sondaki kodlayıcı dolgusu (MP4/AAC).
    trim: Option<Trim>,
}

impl Decoder {
    /// Dosyayı açar, biçimini tanır ve çözücüyü hazırlar.
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        let mut format = probe(path)?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or(AudioError::NoAudioTrack)?;
        let track_id = track.id;
        let num_frames = track.num_frames;
        let time_base = track.time_base;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(AudioError::NoAudioTrack)?
            .clone();

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())?;

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| AudioError::Unsupported("örnekleme hızı bilinmiyor".to_owned()))?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count())
            .filter(|&c| c > 0)
            .ok_or_else(|| AudioError::Unsupported("kanal sayısı bilinmiyor".to_owned()))?;

        // MP3 ve Vorbis'te dolguyu çözücü atar; MP4'te bilgiyi dosyadan biz okuruz.
        let is_mp4 = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| MP4_EXTENSIONS.iter().any(|m| e.eq_ignore_ascii_case(m)));
        let trim = if is_mp4 {
            gapless::mp4_trim(path, sample_rate)
        } else {
            None
        };
        let playable_frames = trim.and_then(|t| t.frames).or(num_frames);

        let tags = read_tags(format.as_mut());
        let file_name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let info = TrackInfo {
            path: path.to_path_buf(),
            file_name,
            title: tags.title,
            artist: tags.artist,
            album: tags.album,
            album_artist: tags.album_artist,
            track_number: tags.track_number,
            disc_number: tags.disc_number,
            codec: decoder.codec_info().short_name.to_owned(),
            sample_rate,
            channels,
            duration_secs: playable_frames.map(|n| n as f64 / f64::from(sample_rate)),
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            info,
            buffer: Vec::new(),
            skip_until: None,
            trim,
        })
    }

    /// Şarkı bilgileri.
    pub fn info(&self) -> &TrackInfo {
        &self.info
    }

    /// Şarkıda verilen saniyeye atlar ve gerçekten atlanan kareyi döndürür.
    ///
    /// Biçimler genellikle hedefin biraz öncesine atlayabilir; aradaki fazla
    /// örnekler sonraki [`Decoder::next_chunk`] çağrılarında atılır.
    pub fn seek(&mut self, seconds: f64) -> Result<u64, AudioError> {
        let rate = f64::from(self.info.sample_rate);
        let last = self
            .info
            .duration_secs
            .map_or(f64::MAX, |d| (d - 1.0 / rate).max(0.0));
        let target = if seconds.is_finite() {
            seconds.clamp(0.0, last)
        } else {
            0.0
        };
        // Akış zamanı = şarkı zamanı + baştaki kodlayıcı dolgusu.
        let stream_target = target + self.trim.map_or(0.0, |t| t.delay as f64 / rate);
        let time = Time::try_from_secs_f64(stream_target)
            .ok_or_else(|| AudioError::Seek("geçersiz konum".to_owned()))?;
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| AudioError::Seek(e.to_string()))?;
        self.decoder.reset();
        self.skip_until = self.time_base.map(|_| stream_target);
        Ok((target * rate).round() as u64)
    }

    /// Sonraki örnek dilimini döndürür; şarkı bittiyse `None`.
    ///
    /// Bozuk tek bir paket şarkıyı durdurmaz: atlanır ve sonrakine geçilir.
    pub fn next_chunk(&mut self) -> Result<Option<&[Sample]>, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                // Bazı biçimlerde dosya sonu G/Ç hatası olarak bildirilir.
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None)
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let packet_start = self
                .time_base
                .and_then(|tb| tb.calc_time(packet.pts))
                .map(|t| t.as_secs_f64());
            match self.decoder.decode(&packet) {
                Ok(audio) => {
                    let frames = audio.frames();
                    if frames == 0 {
                        continue;
                    }
                    self.buffer.resize(audio.samples_interleaved(), 0.0);
                    audio.copy_to_slice_interleaved(&mut self.buffer);
                    // Bozuk kayan noktalı kayıtlarda NaN ya da sonsuz değer olabilir:
                    // filtrelerin belleğine girerse şarkının geri kalanı bozulur. Sessizlik say.
                    for sample in &mut self.buffer {
                        if !sample.is_finite() {
                            *sample = 0.0;
                        }
                    }

                    // Paketin çalınacak kısmı [from, to): sarma hedefinden ve baştaki
                    // dolgudan önceki örnekler, sondaki dolgu da atılır.
                    let (mut from, mut to) = (0usize, frames);
                    if let Some(start) = packet_start {
                        let rate = f64::from(self.info.sample_rate);
                        let start = (start * rate).round() as i64;
                        let delay = self.trim.map_or(0, |t| t.delay as i64);
                        let skip_to = self
                            .skip_until
                            .map_or(0, |t| (t * rate).round() as i64)
                            .max(delay);
                        from = (skip_to - start).clamp(0, frames as i64) as usize;
                        if let Some(length) = self.trim.and_then(|t| t.frames) {
                            let end = delay + length as i64;
                            if start >= end {
                                return Ok(None);
                            }
                            to = (end - start).clamp(0, frames as i64) as usize;
                        }
                        if from >= to {
                            continue;
                        }
                        self.skip_until = None;
                    }
                    let channels = self.info.channels;
                    return Ok(Some(&self.buffer[from * channels..to * channels]));
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Kütüphane ve ekran için okunan etiketler.
#[derive(Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
}

/// Etiketleri okur (ID3, Vorbis yorumları, MP4 vb.). Aynı etiket birden çok
/// kez varsa ilki alınır.
///
/// Eski tip ID3v1 etiketi yalnızca yeni tip etiketlerde olmayan alanlar için kullanılır
/// (30 karakterle sınırlıdır, kod sayfası belirtmez). Metni Türkçe harflerle okunur:
/// bkz. [`id3v1_text`].
fn read_tags(format: &mut dyn FormatReader) -> Tags {
    let mut tags = Tags::default();
    let mut metadata = format.metadata();
    // Bütün revizyonlar: dosyanın başındaki (ID3v2) ve sonundaki (ID3v1, APE) etiketler.
    let mut revisions = Vec::new();
    revisions.extend(metadata.current().cloned());
    while metadata.pop().is_some() {
        revisions.extend(metadata.current().cloned());
    }
    // En yeni revizyon önce (eskiden yalnızca o okunuyordu); ID3v1 en sonda.
    revisions.reverse();
    revisions.sort_by_key(|r| r.info.metadata == METADATA_ID_ID3V1);
    for revision in &revisions {
        let legacy = revision.info.metadata == METADATA_ID_ID3V1;
        let text = |value: &str| {
            if legacy {
                non_empty(&id3v1_text(value))
            } else {
                non_empty(value)
            }
        };
        let track_tags = revision
            .per_track
            .iter()
            .flat_map(|t| t.metadata.tags.iter());
        for tag in revision.media.tags.iter().chain(track_tags) {
            match &tag.std {
                Some(StandardTag::TrackTitle(v)) if tags.title.is_none() => {
                    tags.title = text(v);
                }
                Some(StandardTag::Artist(v)) if tags.artist.is_none() => {
                    tags.artist = text(v);
                }
                Some(StandardTag::Album(v)) if tags.album.is_none() => {
                    tags.album = text(v);
                }
                Some(StandardTag::AlbumArtist(v)) if tags.album_artist.is_none() => {
                    tags.album_artist = text(v);
                }
                Some(StandardTag::TrackNumber(n)) if tags.track_number.is_none() => {
                    tags.track_number = u32::try_from(*n).ok().filter(|&n| n > 0);
                }
                Some(StandardTag::DiscNumber(n)) if tags.disc_number.is_none() => {
                    tags.disc_number = u32::try_from(*n).ok().filter(|&n| n > 0);
                }
                _ => {}
            }
        }
    }
    tags
}

/// ID3v1 metni. Etiket kod sayfası belirtmez; symphonia baytları ISO-8859-1 sayar (her
/// bayt bir karakter, geri çevrilebilir). Baytlar geçerli UTF-8 ise (bazı programlar öyle
/// yazar) UTF-8, değilse Türkçe Windows'un kod sayfası Windows-1254 olarak okunur. Bu, Batı
/// Avrupa harflerini de doğru verir; yalnızca Ð, Ý, Þ, ð, ý, þ yerine Ğ, İ, Ş, ğ, ı, ş gelir.
fn id3v1_text(latin1: &str) -> String {
    let bytes: Option<Vec<u8>> = latin1.chars().map(|c| u8::try_from(c).ok()).collect();
    let Some(bytes) = bytes else {
        return latin1.to_owned(); // zaten çözülmüş metin
    };
    match std::str::from_utf8(&bytes) {
        Ok(text) => return text.to_owned(),
        // 30 baytta kesilmiş UTF-8: son harf yarım kalmış, öncesi geçerli.
        Err(e) if e.error_len().is_none() && !bytes[..e.valid_up_to()].is_ascii() => {
            return String::from_utf8_lossy(&bytes[..e.valid_up_to()]).into_owned();
        }
        Err(_) => {}
    }
    bytes.iter().map(|&b| windows_1254(b)).collect()
}

/// Windows-1254 (Türkçe) kod sayfasındaki karakter.
fn windows_1254(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{FFFD}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{FFFD}',
        '\u{FFFD}', '\u{FFFD}', '\u{FFFD}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›',
        'œ', '\u{FFFD}', '\u{FFFD}', 'Ÿ',
    ];
    match byte {
        0x80..=0x9F => HIGH[usize::from(byte - 0x80)],
        0xD0 => 'Ğ',
        0xDD => 'İ',
        0xDE => 'Ş',
        0xF0 => 'ğ',
        0xFD => 'ı',
        0xFE => 'ş',
        _ => char::from(byte),
    }
}

/// Dosyayı açar ve biçimini tanır (ses çözülmez).
fn probe(path: &Path) -> Result<Box<dyn FormatReader>, AudioError> {
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| match e {
            SymphoniaError::IoError(io) => AudioError::Unsupported(io.to_string()),
            other => AudioError::from(other),
        })
}

/// Şarkının içindeki kapak resmi.
#[derive(Debug, Clone, PartialEq)]
pub struct Cover {
    /// Resmin türü, verinin kendisinden tanınır (ör. "image/jpeg").
    pub media_type: &'static str,
    pub data: Vec<u8>,
}

/// Bu boyuttan büyük kapak gösterilmez: arayüze taşınması pahalı, ekranda farkı yok.
const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;

impl Cover {
    /// Arayüzde doğrudan gösterilebilen `data:` adresi.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, base64(&self.data))
    }
}

/// Dosyanın içindeki kapak resmini okur (ID3, FLAC, MP4, Vorbis…): ön kapak öncelikli,
/// yoksa ilk resim. Ses çözülmez. Resim olmayan ya da çok büyük veri atlanır.
pub fn read_cover(path: &Path) -> Result<Option<Cover>, AudioError> {
    let mut format = probe(path)?;
    let mut metadata = format.metadata();
    let Some(revision) = metadata.skip_to_latest() else {
        return Ok(None);
    };
    let rank = |visual: &Visual| match visual.usage {
        Some(StandardVisualKey::FrontCover) => 0,
        None | Some(StandardVisualKey::OtherIcon) => 1,
        Some(_) => 2,
    };
    let best = revision
        .media
        .visuals
        .iter()
        .chain(
            revision
                .per_track
                .iter()
                .flat_map(|t| t.metadata.visuals.iter()),
        )
        .filter_map(|v| {
            let media_type = image_type(&v.data).filter(|_| v.data.len() <= MAX_COVER_BYTES)?;
            Some((rank(v), media_type, v))
        })
        .min_by_key(|(rank, _, _)| *rank);
    Ok(best.map(|(_, media_type, visual)| Cover {
        media_type,
        data: visual.data.to_vec(),
    }))
}

/// Verinin resim türü (ilk baytlarından; etiketteki tür bilgisine güvenilmez).
fn image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.len() > 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Base64 (RFC 4648, dolgulu).
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= chunk.len() {
                char::from(ALPHABET[((n >> shift) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::{sine, temp_path, write_wav};

    /// 32 bit kayan noktalı mono WAV (biçim 3) yazar: bozuk dosyaları taklit etmek için.
    fn write_float_wav(path: &std::path::Path, samples: &[f32]) {
        let data_len = (samples.len() * 4) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn bozuk_sayilar_sessizlige_cevrilir() {
        // Bozuk bir kayıt: araya NaN ve sonsuz değerler karışmış.
        let path = temp_path("bozuk-float.wav");
        let mut samples = vec![0.25f32; 4_000];
        samples[100] = f32::NAN;
        samples[200] = f32::INFINITY;
        samples[300] = f32::NEG_INFINITY;
        write_float_wav(&path, &samples);
        let mut decoder = Decoder::open(&path).unwrap();
        let mut decoded = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            decoded.extend_from_slice(chunk);
        }
        assert_eq!(decoded.len(), samples.len());
        assert!(
            decoded.iter().all(|s| s.is_finite()),
            "geçersiz sayı kalmamalı"
        );
        assert_eq!(decoded[100], 0.0);
        assert_eq!(decoded[200], 0.0);
        assert_eq!(decoded[300], 0.0);
        assert!((decoded[101] - 0.25).abs() < 1e-6);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn wav_dosyasini_dogru_cozer() {
        let path = temp_path("cozme.wav");
        let frames = 22_050; // 0,5 saniye
        write_wav(&path, 44_100, 2, frames, |frame, channel| {
            sine(440.0, 44_100, frame) * if channel == 0 { 0.5 } else { -0.5 }
        });

        let mut decoder = Decoder::open(&path).unwrap();
        let info = decoder.info().clone();
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.file_name, "cozme");
        assert!((info.duration_secs.unwrap() - 0.5).abs() < 1e-6);

        let mut samples = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            samples.extend_from_slice(chunk);
        }
        assert_eq!(samples.len(), frames * 2);

        // 16 bit'lik nicemleme hatası payıyla karşılaştır.
        for frame in [0, 1, 100, 12_345, frames - 1] {
            let expected = sine(440.0, 44_100, frame) * 0.5;
            assert!((samples[frame * 2] - expected).abs() < 1e-4, "kare {frame}");
            assert!(
                (samples[frame * 2 + 1] + expected).abs() < 1e-4,
                "kare {frame}"
            );
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn sarma_ornek_hassasiyetinde_calisir() {
        let path = temp_path("sarma.wav");
        let rate = 8_000;
        let frames = 24_000; // 3 saniye
                             // Her kare kendi sırasını taşır: kaç saniyeye atlandığı örnekten okunabilir.
        write_wav(&path, rate, 1, frames, |frame, _| {
            frame as f64 / frames as f64
        });

        let mut decoder = Decoder::open(&path).unwrap();
        let landed = decoder.seek(1.25).unwrap();
        assert_eq!(landed, 10_000);

        let mut samples = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            samples.extend_from_slice(chunk);
        }
        // Atlanan noktadan sona kadar tam olarak kalan örnekler gelir.
        assert_eq!(samples.len(), frames - 10_000);
        assert!(
            (samples[0] - 10_000.0 / frames as f64).abs() < 1e-4,
            "ilk örnek {}",
            samples[0]
        );

        // Sona taşan ve negatif konumlar sınırlanır.
        assert!(decoder.seek(99.0).unwrap() < frames as u64);
        assert_eq!(decoder.seek(-5.0).unwrap(), 0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn olmayan_dosya_acma_hatasi_verir() {
        let error = Decoder::open(Path::new("/olmayan/klasor/sarki.mp3"))
            .err()
            .unwrap();
        assert!(matches!(error, AudioError::Open(_)));
    }

    #[test]
    fn ses_olmayan_dosya_desteklenmiyor_hatasi_verir() {
        let path = temp_path("metin.mp3");
        std::fs::write(&path, b"bu bir ses dosyasi degil".repeat(100)).unwrap();
        let error = Decoder::open(&path).err().unwrap();
        assert!(matches!(error, AudioError::Unsupported(_)), "{error:?}");
        std::fs::remove_file(path).ok();
    }

    /// Kapak testleri için küçük "resimler" (yalnızca ilk baytları gerçek; tür bunlardan tanınır).
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR-kapak";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];

    fn fixture(ext: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(format!("uc-uc-ton.{ext}"))
    }

    /// Resimli ID3v2.3 etiketi (APIC çerçeveleri: tür, MIME, veri).
    fn id3_with_pictures(pictures: &[(u8, &str, &[u8])]) -> Vec<u8> {
        let mut frames = Vec::new();
        for (kind, mime, data) in pictures {
            let mut body = vec![0u8]; // metin kodlaması: ISO-8859-1
            body.extend_from_slice(mime.as_bytes());
            body.push(0);
            body.push(*kind);
            body.push(0); // açıklama yok
            body.extend_from_slice(data);
            frames.extend_from_slice(b"APIC");
            frames.extend_from_slice(&(body.len() as u32).to_be_bytes());
            frames.extend_from_slice(&[0, 0]);
            frames.extend_from_slice(&body);
        }
        let size = frames.len() as u32;
        let mut tag = b"ID3\x03\x00\x00".to_vec();
        tag.extend([21, 14, 7, 0].map(|shift| ((size >> shift) & 0x7f) as u8));
        tag.extend(frames);
        tag
    }

    /// Test MP3'ünün kendi etiketini resimli etiketle değiştirir (ses aynen kalır).
    fn mp3_with_pictures(pictures: &[(u8, &str, &[u8])]) -> PathBuf {
        let original = std::fs::read(fixture("mp3")).unwrap();
        assert_eq!(&original[..3], b"ID3");
        let old = original[6..10]
            .iter()
            .fold(0usize, |size, &b| (size << 7) | usize::from(b & 0x7f));
        let mut bytes = id3_with_pictures(pictures);
        bytes.extend_from_slice(&original[10 + old..]);
        let path = temp_path("kapakli.mp3");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// Test FLAC'ına ön kapaklı bir PICTURE bloğu ekler (STREAMINFO'nun ardına).
    fn flac_with_cover(data: &[u8]) -> PathBuf {
        let original = std::fs::read(fixture("flac")).unwrap();
        assert_eq!(&original[..4], b"fLaC");
        let header = original[4];
        let end = 8 + u32::from_be_bytes([0, original[5], original[6], original[7]]) as usize;
        let mut body = Vec::new();
        body.extend_from_slice(&3u32.to_be_bytes()); // ön kapak
        body.extend_from_slice(&9u32.to_be_bytes());
        body.extend_from_slice(b"image/png");
        body.extend_from_slice(&0u32.to_be_bytes()); // açıklama yok
        for value in [1u32, 1, 32, 0] {
            body.extend_from_slice(&value.to_be_bytes());
        }
        body.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        let mut bytes = b"fLaC".to_vec();
        bytes.push(header & 0x7f); // STREAMINFO artık son blok değil
        bytes.extend_from_slice(&original[5..end]);
        bytes.push(6 | (header & 0x80)); // PICTURE; STREAMINFO sondaysa artık o son
        bytes.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        bytes.extend(body);
        bytes.extend_from_slice(&original[end..]);
        let path = temp_path("kapakli.flac");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn kapak_okunur_on_kapak_oncelikli() {
        // Önce "diğer" (JPEG), sonra ön kapak (PNG): ön kapak seçilir.
        let path = mp3_with_pictures(&[(0, "image/jpeg", JPEG), (3, "image/png", PNG)]);
        let cover = read_cover(&path).unwrap().unwrap();
        assert_eq!(cover.media_type, "image/png");
        assert_eq!(cover.data, PNG);
        // Ses yine çözülür (etiket değişti, ses aynı).
        assert!(Decoder::open(&path).is_ok());
        std::fs::remove_file(path).ok();

        // Etiketteki tür yanlış olsa da veriden tanınır.
        let path = mp3_with_pictures(&[(3, "image/jpg", JPEG)]);
        assert_eq!(read_cover(&path).unwrap().unwrap().media_type, "image/jpeg");
        std::fs::remove_file(path).ok();

        let path = flac_with_cover(PNG);
        let cover = read_cover(&path).unwrap().unwrap();
        assert_eq!(
            (cover.media_type, cover.data.as_slice()),
            ("image/png", PNG)
        );
        assert!(Decoder::open(&path).is_ok());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn kapak_yoksa_ya_da_resim_degilse_bos() {
        for ext in ["mp3", "flac", "ogg", "m4a"] {
            assert_eq!(read_cover(&fixture(ext)).unwrap(), None, "{ext}");
        }
        let path = mp3_with_pictures(&[(3, "image/png", b"resim degil")]);
        assert_eq!(read_cover(&path).unwrap(), None);
        std::fs::remove_file(path).ok();
    }

    /// Test MP3'ünün ID3v2 etiketini atar, sonuna verilen 30 baytlık alanlarla eski tip
    /// (ID3v1) etiket ekler: başlık, sanatçı, albüm.
    fn mp3_with_id3v1(title: &[u8], artist: &[u8], album: &[u8]) -> PathBuf {
        let original = std::fs::read(fixture("mp3")).unwrap();
        let old = original[6..10]
            .iter()
            .fold(0usize, |size, &b| (size << 7) | usize::from(b & 0x7f));
        append_id3v1(original[10 + old..].to_vec(), title, artist, album)
    }

    fn append_id3v1(mut bytes: Vec<u8>, title: &[u8], artist: &[u8], album: &[u8]) -> PathBuf {
        let field = |text: &[u8]| {
            let mut out = [0u8; 30];
            out[..text.len()].copy_from_slice(text);
            out
        };
        bytes.extend_from_slice(b"TAG");
        bytes.extend_from_slice(&field(title));
        bytes.extend_from_slice(&field(artist));
        bytes.extend_from_slice(&field(album));
        bytes.extend_from_slice(b"2026");
        bytes.extend_from_slice(&[0u8; 30]); // yorum
        bytes.push(255); // tür yok
        let path = temp_path("id3v1.mp3");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn yalnizca_eski_tip_etiketli_mp3_adi_okunur() {
        // #27: yalnızca ID3v1 taşıyan MP3'lerde listede dosya adı görünüyordu.
        let path = mp3_with_id3v1(b"Gece Yolu", b"Lyraska Test", b"Deneme");
        let info = Decoder::open(&path).unwrap().info().clone();
        assert_eq!(info.title.as_deref(), Some("Gece Yolu"));
        assert_eq!(info.artist.as_deref(), Some("Lyraska Test"));
        assert_eq!(info.album.as_deref(), Some("Deneme"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn eski_tip_etiketteki_turkce_harfler_dogru_okunur() {
        // ID3v1 kod sayfası belirtmez; Türkçe Windows'ta yazılanlar Windows-1254'tür.
        // "Şarkı Ğİ" → Ş 0xDE, ı 0xFD, Ğ 0xD0, İ 0xDD (Latin-1 okunsa "Þarký ÐÝ" olurdu).
        let path = mp3_with_id3v1(
            b"\xDEark\xFD \xD0\xDD",
            b"\xC7a\xF0r\xFD \xD6z\xFC\xFE",
            "Deneme".as_bytes(),
        );
        let info = Decoder::open(&path).unwrap().info().clone();
        assert_eq!(info.title.as_deref(), Some("Şarkı Ğİ"));
        assert_eq!(info.artist.as_deref(), Some("Çağrı Özüş"));
        std::fs::remove_file(path).ok();

        // Bazı programlar ID3v1'e UTF-8 yazar: o da doğru okunur; 30 baytta yarım kalan
        // son harf atılır.
        let path = mp3_with_id3v1("Gökyüzü".as_bytes(), b"Lyraska", b"");
        let info = Decoder::open(&path).unwrap().info().clone();
        assert_eq!(info.title.as_deref(), Some("Gökyüzü"));
        std::fs::remove_file(path).ok();
        let long = "Çok uzun bir şarkı adı ğğğğ".as_bytes();
        let path = mp3_with_id3v1(&long[..30], b"Lyraska", b"");
        let info = Decoder::open(&path).unwrap().info().clone();
        assert_eq!(info.title.as_deref(), Some("Çok uzun bir şarkı adı ğ"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn yeni_tip_etiket_eski_tipten_onceliklidir() {
        // Hem ID3v2 ("Üç Ton") hem ID3v1 ("Kisa") olan dosyada ID3v2 okunur; ID3v1 yalnızca
        // ID3v2'de olmayan alanı (burada yok) tamamlar.
        let original = std::fs::read(fixture("mp3")).unwrap();
        let path = append_id3v1(original, b"Kisa", b"Baska", b"Eski");
        let info = Decoder::open(&path).unwrap().info().clone();
        assert_eq!(info.title.as_deref(), Some("Üç Ton"));
        assert_eq!(info.artist.as_deref(), Some("Lyraska Test"));
        assert_eq!(info.album.as_deref(), Some("Deneme Albümü"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn windows_1254_turkce_ve_bati_harfleri() {
        assert_eq!(id3v1_text("\u{DE}\u{FD}\u{D0}\u{DD}\u{F0}\u{FE}"), "ŞıĞİğş");
        // Diğer Batı Avrupa harfleri ve noktalama Windows-1252 ile aynı.
        assert_eq!(id3v1_text("Caf\u{E9} \u{80}5 \u{93}a\u{94}"), "Café €5 “a”");
        // Zaten Unicode olan metin (ID3v1 değilse) olduğu gibi kalır.
        assert_eq!(id3v1_text("Şarkı"), "Şarkı");
    }

    #[test]
    fn kapak_data_adresi_olur() {
        let cover = Cover {
            media_type: "image/png",
            data: b"foobar".to_vec(),
        };
        assert_eq!(cover.data_url(), "data:image/png;base64,Zm9vYmFy");
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
        assert_eq!(base64(&[0xFB, 0xFF, 0xBF]), "+/+/");
    }
}
