use crate::pipeline::livestream::LivestreamPipeline;
use crate::pipeline::spotify::SpotifyPipeline;
use crate::pipeline::SpotifyInputSourceSelector;
use anyhow::Error;
use futures::lock;
use gstreamer::prelude::{
    Cast, ElementExt, ElementExtManual, GObjectExtManualGst, GstBinExtManual, GstObjectExt,
    ObjectExt, PadExt, PipelineExt,
};
use gstreamer::{
    init, Bus, Caps, ClockTime, Element, ElementFactory, FlowSuccess, Pipeline, State,
    StateChangeSuccess,
};
use gstreamer_app::{gst, AppSrc, AppSrcCallbacks};
use gstreamer_rtsp::RTSPLowerTrans;
use log::error;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::thread::{current, sleep, spawn};
use std::time::{Duration, Instant};

pub struct MainPipeline {
    pub pipeline: Pipeline,
    pub livestream: LivestreamPipeline,
    pub spotify: SpotifyPipeline,
    pub elements: MainPipelineElements,
}

pub struct MainPipelineElements {
    audio_mixer: Element,
    queue: Element,
    aac_encoder: Element,
    stereo_filter: Element,
    mp4_mux: Element,
    rtsp_sink: Element,
}

impl MainPipeline {
    pub fn new() -> Result<Self, Error> {
        init()?;

        let pipeline = Pipeline::new();

        let livestream = LivestreamPipeline::new()?;
        let mut spotify = SpotifyPipeline::new()?;
        let common = MainPipelineElements::new()?;

        livestream.add_to_pipeline(&pipeline)?;
        common.add_to_pipeline(&pipeline)?;
        spotify.add_to_pipeline(&pipeline)?;

        livestream.link_elements()?;
        spotify.link_elements()?;
        common.link_elements()?;

        livestream
            .cap_filter
            .link(&common.audio_mixer)
            .expect("Failed to link livestream to audio mixer");

        spotify
            .audio_resample
            .link(&common.audio_mixer)
            .expect("Failed to link audio mixer");
        Self::connect_dynamic_pads(&livestream)?;

        Self::auto_switch_silence_fallback(
            &spotify.queue,
            &spotify.input_selector,
            &spotify.input_source,
        );

        pipeline.set_latency(ClockTime::from_mseconds(200));

        Ok(Self {
            pipeline,
            livestream,
            spotify,
            elements: common,
        })
    }

    fn auto_switch_silence_fallback(
        queue: &Element,
        input_selector: &Element,
        source_selector: &Arc<Mutex<SpotifyInputSourceSelector>>,
    ) {
        // Spawn a thread to monitor the buffer every 500ms.
        let queue_clone = queue.clone();
        let input_selector_clone = input_selector.clone();
        let selector_clone = source_selector.clone();

        spawn(move || loop {
            monitor_buffer(&queue_clone, &input_selector_clone, &selector_clone);
            sleep(Duration::from_millis(500));
        });
    }

    fn connect_dynamic_pads(livestream: &LivestreamPipeline) -> Result<(), Error> {
        // Clone elements for closure
        let depay_clone = livestream.depay.clone();
        livestream.source.connect_pad_added(move |_src, src_pad| {
            let src_pad_caps = src_pad.current_caps().unwrap();
            let src_pad_structure = src_pad_caps.structure(0).unwrap();

            if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
                if media_type == "audio" {
                    let sink_pad = depay_clone
                        .static_pad("sink")
                        .expect("Failed to get sink pad");
                    if let Err(err) = src_pad.link(&sink_pad) {
                        error!("Failed to link livestream_source audio: {}", err);
                    }
                }
            }
        });

        let queue_clone = livestream.queue.clone();
        livestream.decoder.connect_pad_added(move |_, src_pad| {
            let sink_pad = queue_clone
                .static_pad("sink")
                .expect("Failed to get sink pad from livestream_queue");

            if let Err(err) = src_pad.link(&sink_pad) {
                error!(
                    "Failed to link livestream_decoder to livestream_queue: {}",
                    err
                );
            }
        });

        Ok(())
    }

    pub fn set_state(&self, state: State) -> Result<StateChangeSuccess, Error> {
        self.pipeline.set_state(state)?;
        Ok(StateChangeSuccess::Success)
    }

    pub fn get_bus(&self) -> Option<Bus> {
        self.pipeline.bus()
    }
}

impl MainPipelineElements {
    fn new() -> Result<Self, Error> {
        let audio_mixer = ElementFactory::make_with_name("audiomixer", Some("AudioMixer"))
            .expect("Could not create audio_mixer element.");
        let queue = ElementFactory::make_with_name("queue2", Some("AudioMixerQueue"))
            .expect("Could not create audio_mixer element.");
        let aac_encoder = ElementFactory::make_with_name("avenc_aac", Some("CommonEncoder"))
            .expect("Could not create aac_encoder element.");
        let stereo_filter =
            ElementFactory::make_with_name("capsfilter", Some("CommonStereoFilter"))
                .expect("Could not create stereo_filter element.");
        let mp4_mux = ElementFactory::make_with_name("mp4mux", Some("mp4_mux"))
            .expect("Could not create mp4_mux element.");
        let rtsp_sink = ElementFactory::make_with_name("rtspclientsink", Some("rtsp_sink"))
            .expect("Could not create rtsp_sink element.");

        // let rtsp_sink = ElementFactory::make_with_name("autoaudiosink", Some("rtsp_sink"))
        //     .expect("Could not create rtsp_sink element.");

        queue.set_property("max-size-buffers", &0u32);
        // queue.set_property("max-size-bytes", &0u32);
        // queue.set_property("max-size-time", &0u32);
        queue.set_property("use-buffering", &true);

        // mp3_encoder.set_property("bitrate", &320);
        rtsp_sink.set_property("location", &"rtsp://10.0.0.21:8554/sleep");
        stereo_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw").field("channels", &2).build(),
        );

        Ok(Self {
            audio_mixer,
            queue,
            aac_encoder,
            stereo_filter,
            mp4_mux,
            rtsp_sink,
        })
    }

    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        pipeline.add_many(&[
            &self.audio_mixer,
            &self.stereo_filter,
            &self.queue,
            &self.aac_encoder,
            &self.mp4_mux,
            &self.rtsp_sink,
        ])?;
        Ok(())
    }

    fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[
            &self.audio_mixer,
            &self.queue,
            &self.stereo_filter,
            &self.aac_encoder,
            &self.rtsp_sink,
        ])?;
        Ok(())
    }
}

pub fn monitor_buffer(
    queue: &Element,
    input_selector: &Element,
    source_selector: &Arc<Mutex<SpotifyInputSourceSelector>>,
) {
    let max_bytes: u32 = queue.property::<u32>("max-size-bytes");
    let current_bytes: u32 = queue.property::<u32>("current-level-bytes");

    // log::trace!("current_bytes: {}/{}", current_bytes, max_bytes);
    if current_bytes < max_bytes * 10 / 100 {
        let mut current = source_selector.lock().unwrap();
        if matches!(*current, SpotifyInputSourceSelector::Spotify) {
            // Switch to silence branch (typically sink pad "1")
            for pad in input_selector.pads() {
                if pad.name().contains("sink") && pad.name().contains("1") {
                    log::debug!("Switching to silence due to low buffer");
                    input_selector.set_property("active-pad", &pad);
                    *current = SpotifyInputSourceSelector::Silence;
                    break;
                }
            }
        }
    } else if current_bytes >= max_bytes {
        let mut current = source_selector.lock().unwrap();
        if matches!(*current, SpotifyInputSourceSelector::Silence) {
            // Switch back to Spotify (typically sink pad "0")
            for pad in input_selector.pads() {
                if pad.name().contains("sink") && pad.name().contains("0") {
                    log::debug!("Buffer healthy; switching to appsrc");
                    input_selector.set_property("active-pad", &pad);
                    *current = SpotifyInputSourceSelector::Spotify;
                    break;
                }
            }
        }
    }
}
