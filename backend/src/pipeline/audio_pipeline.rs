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
    aac_encoder: Element,
    stereo_filter: Element,
    mp4_mux: Element,
    rtsp_sink: Element,
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
        let common = AudioPipelineElements::new(&settings.rtsp_server)?;

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

        pipeline.set_latency(ClockTime::from_mseconds(200));

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
    fn new(rtsp_server_url: &str) -> Result<Self, Error> {
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
        queue.set_property("use-buffering", &true);

        rtsp_sink.set_property("location", rtsp_server_url);
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
            &self.stereo_filter,
            &self.queue,
            &self.aac_encoder,
            &self.rtsp_sink,
        ])?;
        Ok(())
    }
}
