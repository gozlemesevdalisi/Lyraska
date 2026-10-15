//! Kör dinleme testi (ABX): iki dosyanın aynı bölümünü, hangisinin çaldığını bilmeden karşılaştırma.
//!
//! İki dosyanın (ör. aynı şarkının FLAC'ı ve MP3'ü) seçilen bölümü belleğe çözülür ve aygıtın hızına
//! çevrilir. Bölümler örnek düzeyinde hizalanır (MP3 kodlayıcısının gecikmesi gibi farklar çapraz
//! ilintiyle bulunur) ve ses yükseklikleri eşitlenir (yalnızca kısarak: yüksek olan duyulmasın diye).
//! Çalarken A, B ve X arasında aynı konumda, kısa bir geçişle değiştirilir; X her denemede rastgele
//! A ya da B'dir. Ses işlenmez: ekolayzer, eşitleme ve kulaklık düzeltmesi atlanır.
//!
//! Sonuç tek yönlü binom sınamasıyla verilir: rastgele tahminle en az bu kadar doğru bulma olasılığı.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

use super::decode::{Decoder, TrackInfo};
use super::loudness::LoudnessMeter;
use super::output::{self, OutputMode, OutputSpec};
use super::player::{wait_for_fade_out, wait_for_prefill, SharedState, FADE_OUT_TIMEOUT};
use super::render::RenderControls;
use super::resample::Converter;
use super::{AudioError, Sample};

/// Bölümün en kısa ve en uzun süresi (saniye).
pub const MIN_CLIP_SECONDS: f64 = 2.0;
pub const MAX_CLIP_SECONDS: f64 = 60.0;
/// Deneme sayısının sınırları; varsayılan 16 (12 doğru ≈ %3,8 rastlantı olasılığı).
pub const MIN_TRIALS: usize = 4;
pub const MAX_TRIALS: usize = 50;
/// İki dosya arasında aranan en büyük kayma (MP3 kodlayıcı gecikmesi ~25–50 ms).
const MAX_LAG_SECONDS: f64 = 0.5;
/// Hizalamada kullanılan en uzun parça (hesap süresini sınırlar).
const ALIGN_WINDOW_SECONDS: f64 = 10.0;
/// Bu benzerliğin altındaki iki dosya aynı kayıt sayılmaz.
const MIN_SIMILARITY: f64 = 0.5;
/// A, B ve X arasında geçiş süresi: aynı konumda kısa bir çapraz geçiş, tık olmasın.
const SWITCH_SECONDS: f64 = 0.010;
/// Bölüm başa dönerken kısma ve açma süresi.
const LOOP_FADE_SECONDS: f64 = 0.015;
/// Eşitlemeden sonra izin verilen en yüksek tepe (iki sürüm aynı oranda kısılır).
const PEAK_LIMIT: f64 = 0.99;
/// Halka tampon: kısa tutulur, A/B/X geçişi ~100 ms içinde duyulsun.
const RING_SECONDS: f64 = 0.08;
/// Besleme iş parçacığının bir turda yazdığı en fazla kare ve tampon doluyken beklemesi.
const BLOCK_FRAMES: usize = 256;
const FEED_IDLE: Duration = Duration::from_millis(2);

/// Dinlenen sürüm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AbxSource {
    A,
    B,
}

/// Kullanıcının seçtiği: A, B ya da kimliği gizli X.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AbxChoice {
    A,
    B,
    X,
}

impl AbxSource {
    fn to_u8(self) -> u8 {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }

    fn from_u8(value: u8) -> Self {
        if value == 1 {
            Self::B
        } else {
            Self::A
        }
    }
}

/// Testin sonucu (bütün denemeler bitince).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AbxResult {
    pub correct: usize,
    pub total: usize,
    /// Rastgele tahminle en az bu kadar doğru bulma olasılığı (tek yönlü binom sınaması).
    pub p_value: f64,
}

/// Arayüzün gösterdiği durum.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AbxStatus {
    /// Test kurulu mu?
    pub active: bool,
    pub a_name: String,
    pub b_name: String,
    /// Bölümün şarkıdaki başlangıcı ve süresi (saniye).
    pub start_secs: f64,
    pub clip_secs: f64,
    /// Bölümün içindeki duyulan konum (saniye).
    pub position_secs: f64,
    /// Seçili olan (duraklatılmışsa `None`).
    pub playing: Option<AbxChoice>,
    /// Yanıtlanan deneme sayısı ve toplam.
    pub answered: usize,
    pub total: usize,
    /// Bütün denemeler bitince sonuç; öncesinde `None` (sonuç ara sıra görülmesin: yanlılık).
    pub result: Option<AbxResult>,
    /// Eşitlemeden önce A'nın B'den ne kadar yüksek olduğu (dB, LUFS farkı).
    pub level_diff_db: f64,
    /// B'nin A'ya göre gecikmesi (ms); hizalamada düzeltildi.
    pub offset_ms: f64,
    /// Hizalanmış iki bölümün benzerliği (0–1, ilinti katsayısı).
    pub similarity: f64,
    /// Aygıtın örnekleme hızı.
    pub sample_rate: u32,
    /// Ses çıkışında hata olduysa nedeni.
    pub error: Option<String>,
}

impl AbxStatus {
    /// Test kurulu değilken.
    pub fn inactive() -> Self {
        Self {
            active: false,
            a_name: String::new(),
            b_name: String::new(),
            start_secs: 0.0,
            clip_secs: 0.0,
            position_secs: 0.0,
            playing: None,
            answered: 0,
            total: 0,
            result: None,
            level_diff_db: 0.0,
            offset_ms: 0.0,
            similarity: 0.0,
            sample_rate: 0,
            error: None,
        }
    }
}

/// Testi kurmak için istenenler.
#[derive(Debug, Clone, PartialEq)]
pub struct AbxRequest {
    pub a: PathBuf,
    pub b: PathBuf,
    pub start_secs: f64,
    pub clip_secs: f64,
    pub trials: usize,
}

/// Rastgele tahminle `total` denemenin en az `correct` tanesini bilme olasılığı:
/// P(K ≥ correct), K ~ Binom(total, 1/2).
pub fn binomial_tail(correct: usize, total: usize) -> f64 {
    if correct == 0 {
        return 1.0;
    }
    if correct > total {
        return 0.0;
    }
    // C(n, k) / 2ⁿ, k = 0'dan başlayarak ardışık oranla; n ≤ 50 için f64 yeterli.
    let n = total as f64;
    let mut term = 0.5f64.powi(total as i32);
    let mut tail = 0.0;
    for k in 0..=total {
        if k >= correct {
            tail += term;
        }
        term *= (n - k as f64) / (k as f64 + 1.0);
    }
    tail.min(1.0)
}

/// Denemeler: X'in her denemede gizli seçimi ve verilen yanıtlar.
pub(crate) struct Trials {
    total: usize,
    answers: Vec<bool>,
    x: AbxSource,
    coin: Box<dyn FnMut() -> bool + Send>,
}

impl Trials {
    pub(crate) fn new(total: usize, mut coin: Box<dyn FnMut() -> bool + Send>) -> Self {
        let x = if coin() { AbxSource::B } else { AbxSource::A };
        Self {
            total: total.clamp(MIN_TRIALS, MAX_TRIALS),
            answers: Vec::new(),
            x,
            coin,
        }
    }

    /// Bu denemede X'in gerçekte ne olduğu.
    pub(crate) fn x(&self) -> AbxSource {
        self.x
    }

    pub(crate) fn finished(&self) -> bool {
        self.answers.len() >= self.total
    }

    /// Yanıtı kaydeder ve sonraki denemenin X'ini çeker. Test bittiyse yok sayılır.
    pub(crate) fn answer(&mut self, guess: AbxSource) {
        if self.finished() {
            return;
        }
        self.answers.push(guess == self.x);
        if !self.finished() {
            self.x = if (self.coin)() {
                AbxSource::B
            } else {
                AbxSource::A
            };
        }
    }

    pub(crate) fn result(&self) -> Option<AbxResult> {
        self.finished().then(|| {
            let correct = self.answers.iter().filter(|&&c| c).count();
            AbxResult {
                correct,
                total: self.total,
                p_value: binomial_tail(correct, self.total),
            }
        })
    }
}

/// Programın her açılışında rastgele anahtarlı (işletim sisteminden) bir yazı-tura: art arda
/// sayaçların özetinin son biti. Kullanıcı sırayı tahmin edemez; yeni bağımlılık gerekmez.
fn random_coin() -> Box<dyn FnMut() -> bool + Send> {
    use std::hash::{BuildHasher, Hasher};
    let state = std::collections::hash_map::RandomState::new();
    let mut counter = 0u64;
    Box::new(move || {
        let mut hasher = state.build_hasher();
        hasher.write_u64(counter);
        counter += 1;
        hasher.finish() & 1 == 1
    })
}

/// Belleğe çözülmüş, hizalanmış ve ses yüksekliği eşitlenmiş iki bölüm (aygıt hızında, iç içe).
#[derive(Debug, Clone)]
pub(crate) struct Clips {
    pub a: Arc<[Sample]>,
    pub b: Arc<[Sample]>,
    pub channels: usize,
    pub rate: u32,
    pub frames: usize,
    pub level_diff_db: f64,
    pub offset_frames: i64,
    pub similarity: f64,
}

/// Dosyanın `start_secs`'ten başlayan `seconds` saniyesini aygıtın hızında çözer. Dosya daha
/// önce biterse eldeki kadarı döner.
fn decode_segment(
    path: &Path,
    start_secs: f64,
    seconds: f64,
    rate: u32,
    channels: usize,
) -> Result<(Vec<Sample>, TrackInfo), AudioError> {
    let mut decoder = Decoder::open(path)?;
    if start_secs > 0.0 {
        decoder.seek(start_secs)?;
    }
    let info = decoder.info().clone();
    let mut converter = Converter::new(info.sample_rate, info.channels, rate, channels)?;
    let wanted = (seconds * f64::from(rate)).ceil() as usize * channels;
    let mut out = Vec::with_capacity(wanted);
    while out.len() < wanted {
        match decoder.next_chunk()? {
            Some(chunk) => out.extend_from_slice(converter.process(chunk)?),
            None => {
                out.extend_from_slice(converter.finish()?);
                break;
            }
        }
    }
    out.truncate(wanted);
    Ok((out, info))
}

/// Kanalların ortalaması.
fn mono(samples: &[Sample], channels: usize) -> Vec<f64> {
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f64>() / channels as f64)
        .collect()
}

/// `a`'nın (kısa) `b` (uzun) içinde en iyi örtüştüğü kayma: r[k] = Σ a[i]·b[i+k] en büyük olan
/// k ∈ 0..=b.len() − a.len(), ve o kaymadaki ilinti katsayısı. Hızlı Fourier dönüşümüyle.
fn find_offset(a: &[f64], b: &[f64]) -> (usize, f64) {
    if a.is_empty() || b.len() < a.len() {
        return (0, 0.0);
    }
    let n = (a.len() + b.len()).next_power_of_two();
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n);
    let inverse = planner.plan_fft_inverse(n);
    let mut fa: Vec<Complex<f64>> = a.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fa.resize(n, Complex::new(0.0, 0.0));
    let mut fb: Vec<Complex<f64>> = b.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fb.resize(n, Complex::new(0.0, 0.0));
    forward.process(&mut fa);
    forward.process(&mut fb);
    let mut r: Vec<Complex<f64>> = fa.iter().zip(&fb).map(|(x, y)| x.conj() * y).collect();
    inverse.process(&mut r);
    let lags = b.len() - a.len();
    let best = (0..=lags)
        .max_by(|&i, &j| r[i].re.total_cmp(&r[j].re))
        .unwrap_or(0);
    let window = &b[best..best + a.len()];
    let dot: f64 = a.iter().zip(window).map(|(x, y)| x * y).sum();
    let ea: f64 = a.iter().map(|x| x * x).sum();
    let eb: f64 = window.iter().map(|x| x * x).sum();
    let similarity = if ea > 0.0 && eb > 0.0 {
        dot / (ea * eb).sqrt()
    } else {
        0.0
    };
    (best, similarity)
}

/// Bölümün bütünleşik ses yüksekliği (LUFS); ölçülemezse (sessiz) `None`.
fn loudness(samples: &[Sample], channels: usize, rate: u32) -> Option<f64> {
    let mut meter = LoudnessMeter::new(rate, channels);
    for frame in samples.chunks_exact(channels) {
        meter.add(frame);
    }
    meter.finish().map(|l| l.integrated_lufs)
}

fn peak(samples: &[Sample]) -> f64 {
    samples.iter().fold(0.0, |m, x| m.max(x.abs()))
}

/// İki dosyanın aynı bölümünü hazırlar: çözer, hizalar, ses yüksekliğini eşitler.
pub(crate) fn prepare(
    request: &AbxRequest,
    rate: u32,
    channels: usize,
) -> Result<Clips, AudioError> {
    let clip_secs = request
        .clip_secs
        .clamp(MIN_CLIP_SECONDS, MAX_CLIP_SECONDS);
    let start = if request.start_secs.is_finite() {
        request.start_secs.max(0.0)
    } else {
        0.0
    };
    // B, A'nın her iki yanında kayma payıyla çözülür.
    let b_start = (start - MAX_LAG_SECONDS).max(0.0);
    let (a, _) = decode_segment(&request.a, start, clip_secs, rate, channels)?;
    let (b, _) = decode_segment(
        &request.b,
        b_start,
        clip_secs + (start - b_start) + MAX_LAG_SECONDS,
        rate,
        channels,
    )?;
    let frames = a.len() / channels;
    if (frames as f64) < MIN_CLIP_SECONDS * f64::from(rate) {
        return Err(AudioError::Abx(
            "A dosyasının seçilen yerinde yeterince ses yok (bölüm şarkının sonuna taşıyor olabilir)."
                .to_owned(),
        ));
    }
    // A'nın başı B'de `expected` karesine düşmeli; ± kayma payı içinde aranır.
    let expected = ((start - b_start) * f64::from(rate)).round() as usize;
    let window = frames.min((ALIGN_WINDOW_SECONDS * f64::from(rate)) as usize);
    let lag_frames = (MAX_LAG_SECONDS * f64::from(rate)) as usize;
    let ma = mono(&a[..window * channels], channels);
    let mb = mono(&b, channels);
    let span = (window + expected + lag_frames).min(mb.len());
    let (best, similarity) = find_offset(&ma, &mb[..span]);
    if similarity < MIN_SIMILARITY {
        return Err(AudioError::Abx(format!(
            "İki dosya aynı kayda benzemiyor (benzerlik %{:.0}). Aynı şarkının iki sürümünü seçin.",
            (similarity.max(0.0)) * 100.0
        )));
    }
    // A'nın her karesine B'de karşılık gelen kare kalmalı.
    let frames = frames.min((b.len() / channels).saturating_sub(best));
    let a: Vec<Sample> = a[..frames * channels].to_vec();
    let b: Vec<Sample> = b[best * channels..(best + frames) * channels].to_vec();
    let offset_frames = best as i64 - expected as i64;

    // Ses yüksekliği eşitleme: yüksek olan, alçak olanın düzeyine kısılır.
    let la = loudness(&a, channels, rate);
    let lb = loudness(&b, channels, rate);
    let level_diff_db = match (la, lb) {
        (Some(la), Some(lb)) => la - lb,
        _ => 0.0,
    };
    let (mut ga, mut gb) = if level_diff_db > 0.0 {
        (10f64.powf(-level_diff_db / 20.0), 1.0)
    } else {
        (1.0, 10f64.powf(level_diff_db / 20.0))
    };
    // Eşitlemeden sonra taşan varsa ikisi birlikte kısılır (eşitlik bozulmasın).
    let top = (peak(&a) * ga).max(peak(&b) * gb);
    if top > PEAK_LIMIT {
        ga *= PEAK_LIMIT / top;
        gb *= PEAK_LIMIT / top;
    }
    let a: Arc<[Sample]> = a.iter().map(|x| x * ga).collect();
    let b: Arc<[Sample]> = b.iter().map(|x| x * gb).collect();
    Ok(Clips {
        a,
        b,
        channels,
        rate,
        frames,
        level_diff_db,
        offset_frames,
        similarity,
    })
}

/// İki bölümü döngüde çalan karıştırıcı. Seçim değişince aynı konumda kısa bir çapraz geçiş
/// yapar; bölüm başa dönerken kısıp açar. Bellek ayırmaz.
pub(crate) struct Mixer {
    clips: Clips,
    pos: usize,
    current: AbxSource,
    from: AbxSource,
    fade_left: usize,
    switch_frames: usize,
    loop_fade: usize,
}

impl Mixer {
    pub(crate) fn new(clips: Clips) -> Self {
        let rate = f64::from(clips.rate);
        Self {
            switch_frames: ((SWITCH_SECONDS * rate) as usize).max(1),
            loop_fade: ((LOOP_FADE_SECONDS * rate) as usize).max(1),
            clips,
            pos: 0,
            current: AbxSource::A,
            from: AbxSource::A,
            fade_left: 0,
        }
    }

    fn sample(&self, source: AbxSource, index: usize) -> Sample {
        match source {
            AbxSource::A => self.clips.a[index],
            AbxSource::B => self.clips.b[index],
        }
    }

    /// Bölüm sınırında kısma: baştan ve sondan `loop_fade` kare içinde doğrusal.
    fn envelope(&self, pos: usize) -> f64 {
        let edge = pos.min(self.clips.frames - 1 - pos) as f64;
        ((edge + 0.5) / self.loop_fade as f64).min(1.0)
    }

    /// `out`'u (kanal sayısının katı) `wanted` sürümüyle doldurur.
    pub(crate) fn fill(&mut self, out: &mut [Sample], wanted: AbxSource) {
        let channels = self.clips.channels;
        for frame in out.chunks_exact_mut(channels) {
            if wanted != self.current && self.fade_left == 0 {
                self.from = self.current;
                self.current = wanted;
                self.fade_left = self.switch_frames;
            }
            let envelope = self.envelope(self.pos);
            let base = self.pos * channels;
            let w = 1.0 - self.fade_left as f64 / self.switch_frames as f64;
            for (c, out) in frame.iter_mut().enumerate() {
                let to = self.sample(self.current, base + c);
                let x = if self.fade_left > 0 {
                    w * to + (1.0 - w) * self.sample(self.from, base + c)
                } else {
                    to
                };
                *out = x * envelope;
            }
            self.fade_left = self.fade_left.saturating_sub(1);
            self.pos += 1;
            if self.pos >= self.clips.frames {
                self.pos = 0;
            }
        }
    }
}

/// Besleme döngüsü: karıştırıcının çıktısını halka tampona yazar (çözme iş parçacığının yerine).
fn feed(
    mut mixer: Mixer,
    mut producer: Producer<Sample>,
    shared: &SharedState,
    selected: &AtomicU8,
) {
    let channels = mixer.clips.channels;
    let mut block = vec![0.0; BLOCK_FRAMES * channels];
    while !shared.stop.load(Ordering::Acquire) {
        let free = producer.slots() / channels;
        if free == 0 {
            shared.prefilled.store(true, Ordering::Release);
            std::thread::sleep(FEED_IDLE);
            continue;
        }
        let n = free.min(BLOCK_FRAMES) * channels;
        let wanted = AbxSource::from_u8(selected.load(Ordering::Acquire));
        mixer.fill(&mut block[..n], wanted);
        if let Ok(chunk) = producer.write_chunk_uninit(n) {
            chunk.fill_from_iter(block[..n].iter().copied());
        }
    }
}

/// Testin ses tarafı: besleme ve çıkış iş parçacıkları.
struct Playback {
    shared: Arc<SharedState>,
    selected: Arc<AtomicU8>,
    feeder: Option<JoinHandle<()>>,
    output: Option<JoinHandle<()>>,
    rate: u32,
    frames: usize,
}

impl Playback {
    fn start(clips: Clips) -> Result<Self, AudioError> {
        let spec = OutputSpec {
            sample_rate: clips.rate,
            channels: clips.channels,
            mode: OutputMode::Shared,
        };
        let capacity = ((RING_SECONDS * f64::from(spec.sample_rate)) as usize).max(BLOCK_FRAMES * 2)
            * spec.channels;
        let (producer, consumer) = RingBuffer::new(capacity);
        // Ses işlenmez: ekolayzer, kulaklık düzeltmesi ve eşitleme atlanır.
        let shared = Arc::new(SharedState::with_controls(
            true,
            RenderControls {
                bit_perfect: true,
                ..RenderControls::default()
            },
        ));
        let selected = Arc::new(AtomicU8::new(AbxSource::A.to_u8()));
        let (rate, frames) = (clips.rate, clips.frames);
        let feeder = {
            let shared = Arc::clone(&shared);
            let selected = Arc::clone(&selected);
            let mixer = Mixer::new(clips);
            std::thread::Builder::new()
                .name("lyraska-abx".to_owned())
                .spawn(move || feed(mixer, producer, &shared, &selected))
                .map_err(|e| AudioError::Abx(e.to_string()))?
        };
        wait_for_prefill(&shared, Duration::from_secs(1));
        let output = match output::spawn(spec, consumer, Arc::clone(&shared)) {
            Ok(handle) => handle,
            Err(error) => {
                shared.stop.store(true, Ordering::Release);
                let _ = feeder.join();
                return Err(error);
            }
        };
        Ok(Self {
            shared,
            selected,
            feeder: Some(feeder),
            output: Some(output),
            rate,
            frames,
        })
    }

    fn play(&self, source: AbxSource) {
        self.selected.store(source.to_u8(), Ordering::Release);
        self.shared.paused.store(false, Ordering::Release);
    }

    fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
    }

    /// Bölüm içindeki duyulan konum (saniye).
    fn position_secs(&self) -> f64 {
        let heard = self.shared.output_heard.load(Ordering::Acquire);
        (heard % self.frames.max(1) as u64) as f64 / f64::from(self.rate.max(1))
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        // Önce yumuşakça kıs, kısılma hoparlöre ulaşınca kapat: tık olmasın.
        self.shared.paused.store(true, Ordering::Release);
        wait_for_fade_out(&self.shared, FADE_OUT_TIMEOUT);
        self.shared.stop.store(true, Ordering::Release);
        if let Some(handle) = self.output.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.feeder.take() {
            let _ = handle.join();
        }
    }
}

/// Kurulu bir kör dinleme testi.
pub struct AbxTest {
    playback: Playback,
    trials: Trials,
    choice: Option<AbxChoice>,
    a_name: String,
    b_name: String,
    start_secs: f64,
    clip_secs: f64,
    level_diff_db: f64,
    offset_ms: f64,
    similarity: f64,
    sample_rate: u32,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl AbxTest {
    /// Testi kurar (iki dosya çözülür, hizalanır, eşitlenir); ses duraklatılmış başlar.
    pub fn start(request: &AbxRequest) -> Result<Self, AudioError> {
        Self::start_with(request, random_coin())
    }

    pub(crate) fn start_with(
        request: &AbxRequest,
        coin: Box<dyn FnMut() -> bool + Send>,
    ) -> Result<Self, AudioError> {
        // İki dosya da stereo çalınır (mono iki kanala çoğaltılır); kanal sayısı farklıysa
        // karşılaştırma anlamsızdır.
        let ca = Converter::output_channels(Decoder::open(&request.a)?.info().channels);
        let cb = Converter::output_channels(Decoder::open(&request.b)?.info().channels);
        if ca != cb {
            return Err(AudioError::Abx(format!(
                "İki dosyanın kanal sayısı farklı ({ca} ve {cb})."
            )));
        }
        let rate = match output::device_info() {
            Some(device) => device.sample_rate,
            None => Decoder::open(&request.a)?.info().sample_rate,
        };
        let clips = prepare(request, rate, ca)?;
        let clip_secs = clips.frames as f64 / f64::from(rate);
        let level_diff_db = clips.level_diff_db;
        let offset_ms = clips.offset_frames as f64 * 1000.0 / f64::from(rate);
        let similarity = clips.similarity;
        let playback = Playback::start(clips)?;
        Ok(Self {
            playback,
            trials: Trials::new(request.trials, coin),
            choice: None,
            a_name: file_name(&request.a),
            b_name: file_name(&request.b),
            start_secs: request.start_secs.max(0.0),
            clip_secs,
            level_diff_db,
            offset_ms,
            similarity,
            sample_rate: rate,
        })
    }

    fn source_of(&self, choice: AbxChoice) -> AbxSource {
        match choice {
            AbxChoice::A => AbxSource::A,
            AbxChoice::B => AbxSource::B,
            AbxChoice::X => self.trials.x(),
        }
    }

    /// Seçileni çalar (aynı konumdan, kısa geçişle).
    pub fn select(&mut self, choice: AbxChoice) {
        self.choice = Some(choice);
        self.playback.play(self.source_of(choice));
    }

    pub fn pause(&mut self) {
        self.choice = None;
        self.playback.pause();
    }

    /// "X, A'dır" ya da "X, B'dir" yanıtı. X çalıyorsa sonraki denemenin X'ine geçer.
    pub fn answer(&mut self, guess: AbxSource) {
        self.trials.answer(guess);
        if let Some(choice) = self.choice {
            self.playback.play(self.source_of(choice));
        }
    }

    pub fn status(&self) -> AbxStatus {
        AbxStatus {
            active: true,
            a_name: self.a_name.clone(),
            b_name: self.b_name.clone(),
            start_secs: self.start_secs,
            clip_secs: self.clip_secs,
            position_secs: self.playback.position_secs(),
            playing: self.choice,
            answered: self.trials.answers.len(),
            total: self.trials.total,
            result: self.trials.result(),
            level_diff_db: self.level_diff_db,
            offset_ms: self.offset_ms,
            similarity: self.similarity,
            sample_rate: self.sample_rate,
            error: self.playback.shared.error(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::output::simulated;
    use crate::audio::test_util::{temp_path, write_wav};

    /// Tekrarlanabilir sözde rastgele ses (müziğe benzer: geniş bantlı, zarflı).
    fn noise(frames: usize, seed: u64) -> Vec<f64> {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (0..frames)
            .map(|i| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                // Üst 31 bit → [0, 2) → [−1, 1): ortalaması sıfır.
                let white = ((state >> 33) as f64 / (1u64 << 30) as f64) - 1.0;
                let envelope = 0.5 + 0.5 * (i as f64 / 9_000.0).sin();
                0.4 * white * envelope
            })
            .collect()
    }

    #[test]
    fn binom_kuyrugu_bilinen_degerleri_verir() {
        assert!((binomial_tail(12, 16) - 2517.0 / 65_536.0).abs() < 1e-12);
        assert!((binomial_tail(8, 16) - 39_203.0 / 65_536.0).abs() < 1e-12);
        assert!((binomial_tail(16, 16) - 1.0 / 65_536.0).abs() < 1e-15);
        assert_eq!(binomial_tail(0, 16), 1.0);
        assert_eq!(binomial_tail(17, 16), 0.0);
    }

    #[test]
    fn kayma_ve_benzerlik_bulunur() {
        let a = noise(20_000, 1);
        let mut b = vec![0.0; 1_234];
        b.extend_from_slice(&a);
        b.extend(vec![0.0; 3_000]);
        let (lag, similarity) = find_offset(&a, &b);
        assert_eq!(lag, 1_234);
        assert!(similarity > 0.999, "{similarity}");
        // Başka bir kayıt: benzerlik düşük.
        let other = noise(30_000, 2);
        let (_, similarity) = find_offset(&a, &other);
        assert!(similarity < 0.2, "{similarity}");
    }

    fn coin_from(bits: Vec<bool>) -> Box<dyn FnMut() -> bool + Send> {
        let mut bits = bits.into_iter().cycle();
        Box::new(move || bits.next().unwrap_or(false))
    }

    #[test]
    fn denemeler_sayilir_sonuc_yalnizca_sonda_verilir() {
        // X sırası: A, B, B, A (ilk çekiliş kurulurken).
        let mut trials = Trials::new(4, coin_from(vec![false, true, true, false]));
        assert_eq!(trials.x(), AbxSource::A);
        trials.answer(AbxSource::A); // doğru
        assert!(trials.result().is_none(), "ara sonuç gösterilmez");
        assert_eq!(trials.x(), AbxSource::B);
        trials.answer(AbxSource::A); // yanlış
        trials.answer(AbxSource::B); // doğru
        trials.answer(AbxSource::A); // doğru
        let result = trials.result().unwrap();
        assert_eq!((result.correct, result.total), (3, 4));
        assert!((result.p_value - 5.0 / 16.0).abs() < 1e-12);
        // Bitince yeni yanıt sayılmaz.
        trials.answer(AbxSource::B);
        assert_eq!(trials.result().unwrap().correct, 3);
    }

    #[test]
    fn rastgele_secim_iki_yana_da_duser() {
        let mut coin = random_coin();
        let heads = (0..1_000).filter(|_| coin()).count();
        assert!((350..=650).contains(&heads), "{heads}");
    }

    fn clips(a: Vec<f64>, b: Vec<f64>, rate: u32) -> Clips {
        let frames = a.len();
        Clips {
            a: a.into(),
            b: b.into(),
            channels: 1,
            rate,
            frames,
            level_diff_db: 0.0,
            offset_frames: 0,
            similarity: 1.0,
        }
    }

    #[test]
    fn gecis_ayni_konumda_ve_tiksiz() {
        let rate = 48_000;
        let n = rate as usize;
        // A ve B aynı ton, B biraz kısık: geçişte konum kaymıyorsa dalga kopmaz.
        let a: Vec<f64> = (0..n).map(|i| 0.5 * (i as f64 * 0.05).sin()).collect();
        let b: Vec<f64> = a.iter().map(|x| x * 0.9).collect();
        let mut mixer = Mixer::new(clips(a.clone(), b.clone(), rate));
        let mut out = vec![0.0; 4_800];
        mixer.fill(&mut out, AbxSource::A);
        let mut switched = vec![0.0; 4_800];
        mixer.fill(&mut switched, AbxSource::B);
        let all: Vec<f64> = out.iter().chain(&switched).copied().collect();
        let jump = all
            .windows(2)
            .skip(1_000)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        // Tonun kendi en büyük adımı ~0,025: geçiş bunu aşmaz.
        assert!(jump < 0.03, "en büyük sıçrama {jump}");
        // Geçiş bitince tam B, aynı konumda.
        let i = 9_000;
        assert!((all[i] - b[i]).abs() < 1e-12);
    }

    #[test]
    fn bolum_basa_donerken_kisilir_ve_acilir() {
        let rate = 48_000;
        let n = 10_000;
        let a = vec![0.5; n];
        let mut mixer = Mixer::new(clips(a.clone(), a, rate));
        let mut out = vec![0.0; 2 * n];
        mixer.fill(&mut out, AbxSource::A);
        assert!(out[n - 1].abs() < 0.05 && out[n].abs() < 0.05, "sınırda sessiz");
        assert!((out[n / 2] - 0.5).abs() < 1e-12, "ortada tam");
        let jump = out.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
        assert!(jump < 0.5 / (LOOP_FADE_SECONDS * f64::from(rate)) * 1.5, "{jump}");
    }

    fn test_files(delay_frames: usize, b_gain: f64) -> (PathBuf, PathBuf, Vec<f64>) {
        let rate = 44_100;
        let frames = 8 * rate as usize;
        let signal = noise(frames + delay_frames, 7);
        let a = temp_path("a.wav");
        let b = temp_path("b.wav");
        write_wav(&a, rate, 2, frames, |i, c| signal[i] * if c == 0 { 1.0 } else { 0.8 });
        write_wav(&b, rate, 2, frames + delay_frames, |i, c| {
            let x = if i < delay_frames { 0.0 } else { signal[i - delay_frames] };
            b_gain * x * if c == 0 { 1.0 } else { 0.8 }
        });
        (a, b, signal)
    }

    #[test]
    fn hazirlik_hizalar_ve_ses_yuksekligini_esitler() {
        // B, MP3 kodlayıcı gecikmesi gibi 1105 kare geç ve 6 dB kısık.
        let (a, b, _) = test_files(1_105, 0.5);
        let request = AbxRequest {
            a,
            b,
            start_secs: 1.0,
            clip_secs: 4.0,
            trials: 16,
        };
        let clips = prepare(&request, 44_100, 2).unwrap();
        assert_eq!(clips.offset_frames, 1_105);
        assert!(clips.similarity > 0.999, "{}", clips.similarity);
        assert!((clips.level_diff_db - 6.02).abs() < 0.1, "{}", clips.level_diff_db);
        // Eşitlenmiş ve hizalanmış iki bölüm örnek düzeyinde aynı (16 bit yuvarlama kadar).
        let diff = clips
            .a
            .iter()
            .zip(clips.b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(diff < 1e-3, "en büyük fark {diff}");
    }

    #[test]
    fn farkli_kayitlar_reddedilir() {
        let (a, _, _) = test_files(0, 1.0);
        let other = temp_path("baska.wav");
        let signal = noise(8 * 44_100, 99);
        write_wav(&other, 44_100, 2, signal.len(), |i, _| signal[i]);
        let request = AbxRequest {
            a,
            b: other,
            start_secs: 0.0,
            clip_secs: 4.0,
            trials: 16,
        };
        let error = AbxTest::start_with(&request, coin_from(vec![false])).err().unwrap();
        assert!(error.to_string().contains("aynı kayda benzemiyor"), "{error}");
    }

    #[test]
    fn test_calar_secimi_degistirir_ve_yanitlari_sayar() {
        simulated::set_device_rate(Some(48_000));
        let (a, b, _) = test_files(500, 0.7);
        let request = AbxRequest {
            a,
            b,
            start_secs: 0.5,
            clip_secs: 3.0,
            trials: 4,
        };
        // X sırası: B, A, A, B.
        let mut test = AbxTest::start_with(&request, coin_from(vec![true, false, false, true])).unwrap();
        let status = test.status();
        assert!(status.active && status.playing.is_none());
        assert_eq!(status.sample_rate, 48_000);
        assert!((status.clip_secs - 3.0).abs() < 0.01);
        assert!((status.offset_ms - 500.0 / 44.1).abs() < 0.2, "{}", status.offset_ms);
        test.select(AbxChoice::X);
        assert_eq!(test.playback.selected.load(Ordering::Acquire), AbxSource::B.to_u8());
        test.answer(AbxSource::B); // doğru; X artık A
        assert_eq!(test.playback.selected.load(Ordering::Acquire), AbxSource::A.to_u8());
        test.select(AbxChoice::B);
        test.answer(AbxSource::B); // yanlış (X = A)
        test.answer(AbxSource::A); // doğru
        assert!(test.status().result.is_none());
        test.answer(AbxSource::B); // doğru
        let result = test.status().result.unwrap();
        assert_eq!((result.correct, result.total), (3, 4));
        // Ses çalıyor: konum ilerliyor, hata yok.
        let before = test.status().position_secs;
        std::thread::sleep(Duration::from_millis(30));
        let status = test.status();
        assert!(status.error.is_none());
        assert!(status.position_secs != before || before > 0.0);
        simulated::set_device_rate(None);
    }
}
