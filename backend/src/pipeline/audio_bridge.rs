use crate::spotify_sink::SinkEvent;
use gstreamer::prelude::{ElementExt, ElementExtManual};
use gstreamer::{Buffer, ClockTime};
use gstreamer_app::AppSrc;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

pub struct AudioBridge {
    app_src: Arc<Mutex<Option<AppSrc>>>,
    _handle: JoinHandle<()>,
}

impl AudioBridge {
    pub fn new(receiver: Receiver<SinkEvent>) -> Self {
        let app_src: Arc<Mutex<Option<AppSrc>>> = Arc::new(Mutex::new(None));
        let app_src_clone = app_src.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let mut timestamp: u64 = 0;

            while let Ok(event) = receiver.recv() {
                match event {
                    SinkEvent::Start => {}
                    SinkEvent::Stop => {}
                    SinkEvent::Packet(samples) => {
                        if samples.is_empty() {
                            continue;
                        }

                        // Get the current app_src, if any
                        let current_src = {
                            let guard = app_src_clone.lock().unwrap();
                            guard.clone()
                        };

                        if let Some(src) = current_src {
                            let byte_len = samples.len() * std::mem::size_of::<f64>();
                            let mut buffer = Buffer::with_size(byte_len)
                                .expect("Failed to allocate buffer for audio data");
                            {
                                let buffer_mut = buffer.get_mut().unwrap();
                                let mut map = buffer_mut
                                    .map_writable()
                                    .expect("Failed to map buffer writable");
                                let sample_bytes = unsafe {
                                    std::slice::from_raw_parts(
                                        samples.as_ptr() as *const u8,
                                        byte_len,
                                    )
                                };
                                map.copy_from_slice(sample_bytes);
                            }

                            let frames = (samples.len() as u64) / 2;
                            let duration_ns = frames * 1_000_000_000 / 44100;

                            {
                                let buffer_mut = buffer.get_mut().unwrap();
                                buffer_mut.set_pts(ClockTime::from_nseconds(timestamp));
                                buffer_mut
                                    .set_duration(ClockTime::from_nseconds(duration_ns));
                            }
                            timestamp += duration_ns;

                            if let Err(err) = src.push_buffer(buffer) {
                                log::warn!("Failed to push buffer to AppSrc: {:?}", err);
                                // If pushing fails, we assume the pipeline is dead or dying.
                                // We don't break the loop, we just wait for a new AppSrc to be set.
                                // But we might want to clear the current one to avoid spamming errors?
                                // For now, let's just log. The server loop should detect the error and replace the AppSrc.
                            }
                        }
                    }
                }
            }
            log::info!("AudioBridge channel closed");
        });

        Self {
            app_src,
            _handle: handle,
        }
    }

    pub fn set_app_src(&self, src: AppSrc) {
        let mut guard = self.app_src.lock().unwrap();
        *guard = Some(src);
    }

    pub fn clear_app_src(&self) {
        let mut guard = self.app_src.lock().unwrap();
        *guard = None;
    }
}
