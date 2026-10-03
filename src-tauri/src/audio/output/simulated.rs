//! Testler için sanal ses çıkışı.
//!
//! Gerçek çıkışla aynı [`Renderer`]'ı kullanır ama bir ses aygıtı yerine halka
//! tamponu hızlandırılmış zamanda tüketir. Böylece oynatıcının bütün akışı
//! (aç, duraklat, sar, durdur, şarkı sonu) her makinede test edilebilir.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::Consumer;

use super::OutputSpec;
use crate::audio::player::SharedState;
use crate::audio::render::Renderer;
use crate::audio::{AudioError, Sample};

/// Her turda tüketilen süre (şarkı zamanı) ve turlar arası bekleme (gerçek zaman):
/// 50 ms ses / 1 ms → yaklaşık 50 kat hız.
const STEP_SECONDS: f64 = 0.05;
const STEP_SLEEP: Duration = Duration::from_millis(1);

pub fn spawn(
    spec: OutputSpec,
    mut source: Consumer<Sample>,
    shared: Arc<SharedState>,
) -> Result<JoinHandle<()>, AudioError> {
    let frames_per_step = (f64::from(spec.sample_rate) * STEP_SECONDS) as usize;
    std::thread::Builder::new()
        .name("lyraska-sanal-cikis".to_owned())
        .spawn(move || {
            let mut renderer =
                Renderer::new(spec.channels, spec.sample_rate, Arc::clone(&shared.eq));
            let mut out = vec![0.0f32; frames_per_step * spec.channels];
            let mut consumed = 0u64;
            while !shared.stop.load(Ordering::Acquire) {
                if shared.decode_done.load(Ordering::Acquire) && source.is_empty() {
                    shared.ended.store(true, Ordering::Release);
                    return;
                }
                let paused = shared.paused.load(Ordering::Acquire);
                let outcome = renderer.render(&mut source, &mut out, paused);
                consumed += outcome.frames_consumed as u64;
                shared
                    .frames_played
                    .store(shared.start_frame + consumed, Ordering::Release);
                std::thread::sleep(STEP_SLEEP);
            }
        })
        .map_err(|e| AudioError::Output(e.to_string()))
}
