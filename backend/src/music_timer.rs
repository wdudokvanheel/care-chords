use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

pub struct MusicVolume {
    volume: Mutex<f64>,
}

impl MusicVolume {
    pub fn new(initial_volume: f64) -> Self {
        Self {
            volume: Mutex::new(initial_volume.clamp(0.0, 1.0)),
        }
    }

    pub fn set_volume(&self, new_volume: f64) {
        let mut volume = self.volume.lock().unwrap();
        *volume = new_volume.clamp(0.0, 1.0);
    }

    pub fn get_volume(&self) -> f64 {
        *self.volume.lock().unwrap()
    }
}

struct SleepTimerInner {
    handle: Option<JoinHandle<()>>,
    initial_volume: f64,
    deadline: Option<Instant>,
}

pub struct SleepTimer {
    inner: TokioMutex<SleepTimerInner>,
    volume: Arc<MusicVolume>,
}

impl SleepTimer {
    pub fn new(volume: Arc<MusicVolume>) -> Self {
        Self {
            inner: TokioMutex::new(SleepTimerInner {
                handle: None,
                initial_volume: volume.get_volume(),
                deadline: None,
            }),
            volume,
        }
    }

    pub async fn set_timer<F, Fut>(&self, delay: Duration, on_elapsed: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut inner = self.inner.lock().await;
        if let Some(handle) = inner.handle.take() {
            handle.abort();
            self.volume.set_volume(inner.initial_volume);
        }

        if delay.is_zero() {
            inner.deadline = None;
            return;
        }

        inner.initial_volume = self.volume.get_volume();
        inner.deadline = Some(Instant::now() + delay);

        let volume = self.volume.clone();
        let initial_volume = inner.initial_volume;
        inner.handle = Some(tokio::spawn(async move {
            sleep(delay).await;
            fade_out_volume(volume.clone(), initial_volume).await;
            on_elapsed().await;

            sleep(Duration::from_secs(1)).await;
            volume.set_volume(initial_volume);
        }));
    }

    pub async fn remaining_time(&self) -> Option<Duration> {
        let inner = self.inner.lock().await;
        if let Some(deadline) = inner.deadline {
            let now = Instant::now();
            if deadline > now {
                return Some(deadline - now);
            }
        }
        None
    }
}

async fn fade_out_volume(volume: Arc<MusicVolume>, initial_volume: f64) {
    log::trace!("Fading out music volume");
    let fade_duration = Duration::from_secs(10);
    let steps = 100;
    let step_duration = fade_duration / steps;

    for step in 0..steps {
        let fraction = (step + 1) as f64 / steps as f64;
        volume.set_volume(initial_volume * (1.0 - fraction));
        sleep(step_duration).await;
    }
}
