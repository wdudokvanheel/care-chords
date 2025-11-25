use crate::app_settings::ApplicationSettings;
use crate::pipeline::monitor_source::MonitorSourcePipeline;
use crate::pipeline::spotify_source::SpotifySourcePipeline;
use anyhow::Error;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstBinExtManual, ObjectExt, PipelineExt};
use gstreamer::{
    Bus, Caps, ClockTime, Element, ElementFactory, Pipeline, State, StateChangeSuccess, init,
};

#[allow(dead_code)]
pub struct AudioPipeline {
    pub gstreamer_pipeline: Pipeline,
    pub monitor: MonitorSourcePipeline,
    pub spotify: SpotifySourcePipeline,
    pub elements: AudioPipelineElements,
}

pub struct AudioPipelineElements {
    audio_mixer: Element,
    queue: Element,
    audio_convert: Element,
    stereo_filter: Element,
    rtp_pay: Element,
    udp_sink: Element,
}

pub trait PipeLineBranch {
    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error>;
    fn link_elements(&self) -> Result<(), Error>;
    fn last_element(&self) -> Element;
}

impl AudioPipeline {
    pub fn new(settings: &ApplicationSettings) -> Result<Self, Error> {
        init()?;

        let pipeline = Pipeline::new();

        let monitor = MonitorSourcePipeline::new(&settings.monitor_url, settings.noise_filter)?;
        let spotify = SpotifySourcePipeline::new()?;
        let common = AudioPipelineElements::new()?;

        monitor.add_to_pipeline(&pipeline)?;
        common.add_to_pipeline(&pipeline)?;
        spotify.add_to_pipeline(&pipeline)?;

        monitor.link_elements()?;
        spotify.link_elements()?;
        common.link_elements()?;

        monitor
            .last_element()
            .link(&common.audio_mixer)
            .expect("Failed to link livestream to audio mixer");

        spotify
            .last_element()
            .link(&common.audio_mixer)
            .expect("Failed to link audio mixer");

        pipeline.set_latency(ClockTime::from_mseconds(1000));

        Ok(Self {
            gstreamer_pipeline: pipeline,
            monitor,
            spotify,
            elements: common,
        })
    }

    pub fn set_state(&self, state: State) -> Result<StateChangeSuccess, Error> {
        self.gstreamer_pipeline.set_state(state)?;
        Ok(StateChangeSuccess::Success)
    }

    pub fn get_bus(&self) -> Option<Bus> {
        self.gstreamer_pipeline.bus()
    }
}

impl AudioPipelineElements {
    fn new() -> Result<Self, Error> {
        let audio_mixer = ElementFactory::make_with_name("audiomixer", Some("AudioMixer"))
            .expect("Could not create audio_mixer element.");
        let queue = ElementFactory::make_with_name("queue2", Some("AudioMixerQueue"))
            .expect("Could not create audio_mixer element.");
        let audio_convert = ElementFactory::make_with_name("audioconvert", Some("AudioConvert"))
            .expect("Could not create audio_convert element.");
        let stereo_filter =
            ElementFactory::make_with_name("capsfilter", Some("CommonStereoFilter"))
                .expect("Could not create stereo_filter element.");
        
        // We use udpsink to send the stream to the local RTSP server
        // Use L16 (Raw Audio) for the bridge to avoid AAC config issues
        let rtp_pay = ElementFactory::make_with_name("rtpL16pay", Some("rtp_pay"))
            .expect("Could not create rtp_pay element.");
        let udp_sink = ElementFactory::make_with_name("udpsink", Some("udp_sink"))
            .expect("Could not create udp_sink element.");

        queue.set_property("max-size-buffers", &0u32);
        queue.set_property("use-buffering", &true);
        queue.set_property("max-size-time", &200_000_000u64); // 200ms to bound latency for AudioMixer

        udp_sink.set_property("host", "127.0.0.1");
        udp_sink.set_property("port", &5000);
        
        // Set explicit latency on the mixer to avoid latency negotiation issues
        // with unbounded sinks (udpsink reports max_latency=0)
        audio_mixer.set_property("latency", &100_000_000u64); // 100ms in nanoseconds

        stereo_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw")
                .field("channels", &2)
                .field("rate", &44100)
                .build(),
        );

        Ok(Self {
            audio_mixer,
            queue,
            audio_convert,
            stereo_filter,
            rtp_pay,
            udp_sink,
        })
    }

    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        pipeline.add_many(&[
            &self.audio_mixer,
            &self.stereo_filter,
            &self.queue,
            &self.audio_convert,
            &self.rtp_pay,
            &self.udp_sink,
        ])?;
        Ok(())
    }

    fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[
            &self.audio_mixer,
            &self.stereo_filter,
            &self.queue,
            &self.audio_convert,
            &self.rtp_pay,
            &self.udp_sink,
        ])?;
        Ok(())
    }
}
