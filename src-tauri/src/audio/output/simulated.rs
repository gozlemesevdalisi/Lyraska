//! Testler için sanal ses çıkışı.
//!
//! Gerçek çıkışla aynı [`Renderer`]'ı kullanır ama bir ses aygıtı yerine halka
//! tamponu hızlandırılmış zamanda tüketir. Böylece oynatıcının bütün akışı
//! (aç, duraklat, sar, durdur, şarkı sonu) her makinede test edilebilir.

use std::cell::Cell;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::Consumer;

use super::{DeviceInfo, FadeWatch, IntFormat, OutputMode, OutputSpec};
use crate::audio::player::SharedState;
use crate::audio::render::Renderer;
use crate::audio::{AudioError, Sample};

/// Her turda tüketilen süre (şarkı zamanı) ve turlar arası bekleme (gerçek zaman):
/// 50 ms ses / 1 ms → yaklaşık 50 kat hız.
const STEP_SECONDS: f64 = 0.05;
const STEP_SLEEP: Duration = Duration::from_millis(1);

thread_local! {
    /// Testin seçtiği sanal aygıt hızı; `None` ise aygıt bilgisi yok sayılır.
    static DEVICE_RATE: Cell<Option<u32>> = const { Cell::new(None) };
    /// Sanal aygıtın özel modda kabul ettiği biçim (`None`: özel mod yok).
    static EXCLUSIVE: Cell<Option<IntFormat>> = const { Cell::new(None) };
    /// Özel mod açılırken aygıt başka programca tutuluyor mu?
    static EXCLUSIVE_BUSY: Cell<bool> = const { Cell::new(false) };
}

/// Bu iş parçacığındaki testler için sanal aygıtın örnekleme hızını ayarlar.
pub fn set_device_rate(rate: Option<u32>) {
    DEVICE_RATE.with(|r| r.set(rate));
}

/// Bu iş parçacığındaki testler için sanal aygıtın özel mod desteğini ayarlar.
/// `busy`: biçim desteklense de akış açılamaz (ör. başka program aygıtı tutuyor).
pub fn set_exclusive(format: Option<IntFormat>, busy: bool) {
    EXCLUSIVE.with(|f| f.set(format));
    EXCLUSIVE_BUSY.with(|b| b.set(busy));
}

pub fn exclusive_format(sample_rate: u32, channels: usize) -> Result<IntFormat, String> {
    let _ = (sample_rate, channels);
    EXCLUSIVE
        .with(Cell::get)
        .ok_or_else(|| "sanal aygıt özel modu desteklemiyor".to_owned())
}

pub fn device_info() -> Option<DeviceInfo> {
    DEVICE_RATE.with(Cell::get).map(|sample_rate| DeviceInfo {
        name: "Sanal aygıt".to_owned(),
        sample_rate,
        channels: 2,
    })
}

pub fn spawn(
    spec: OutputSpec,
    mut source: Consumer<Sample>,
    shared: Arc<SharedState>,
) -> Result<JoinHandle<()>, AudioError> {
    if matches!(spec.mode, OutputMode::Exclusive(_)) && EXCLUSIVE_BUSY.with(Cell::get) {
        return Err(AudioError::Exclusive(
            "aygıtı başka bir program özel modda kullanıyor".to_owned(),
        ));
    }
    let frames_per_step = (f64::from(spec.sample_rate) * STEP_SECONDS) as usize;
    std::thread::Builder::new()
        .name("lyraska-sanal-cikis".to_owned())
        .spawn(move || {
            let mut renderer =
                Renderer::new(spec.channels, spec.sample_rate, shared.controls.clone());
            let mut out: Vec<Sample> = vec![0.0; frames_per_step * spec.channels];
            let mut consumed = 0u64;
            let mut fade = FadeWatch::default();
            while !shared.stop.load(Ordering::Acquire) {
                if shared.decode_done.load(Ordering::Acquire) && source.is_empty() {
                    shared.ended.store(true, Ordering::Release);
                    return;
                }
                let paused = shared.paused.load(Ordering::Acquire);
                let outcome = renderer.render(&mut source, &mut out, paused);
                consumed += outcome.frames_consumed as u64;
                shared.output_heard.store(consumed, Ordering::Release);
                // Sanal aygıtın arabelleği yok: yazılan hemen "çalınır".
                let silent = paused && renderer.gain() == 0.0;
                fade.wrote(frames_per_step, silent, renderer.latency_frames());
                shared.faded_out.store(fade.faded_out(0), Ordering::Release);
                std::thread::sleep(STEP_SLEEP);
            }
        })
        .map_err(|e| AudioError::Output(e.to_string()))
}
