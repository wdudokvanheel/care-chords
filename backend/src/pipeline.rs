use anyhow::Error;
use gstreamer::prelude::{
    Cast, ElementExt, ElementExtManual, GObjectExtManualGst, GstBinExtManual, GstObjectExt,
    ObjectExt, PadExt,
};
use gstreamer::{
    init, Bus, Caps, ClockTime, Element, ElementFactory, FlowSuccess, Pipeline, State,
    StateChangeSuccess,
};
use gstreamer_app::{gst, AppSrc};
use gstreamer_rtsp::RTSPLowerTrans;
use log::error;
use std::thread;
use std::time::Duration;

#[allow(dead_code)]
pub struct StreamPipeline {
    pub pipeline: Pipeline,
    pub livestream: LivestreamElements,
    // pub music: MusicElements,
    pub spotify: SpotifyElements,
    pub common: CommonElements,
}

pub struct SpotifyElements {
    pub app_source: Element,
    audio_convert: Element,
    audio_resample: Element,
    input_selector: Element,
    silent_src: Element,
}

impl StreamPipeline {
    pub fn new() -> Result<Self, Error> {
        init()?;

        let pipeline = Pipeline::new();

        let livestream = LivestreamElements::new()?;
        // let music = MusicElements::new()?;
        let spotify = SpotifyElements::new()?;
        let common = CommonElements::new()?;

        livestream.add_to_pipeline(&pipeline)?;
        // music.add_to_pipeline(&pipeline)?;
        common.add_to_pipeline(&pipeline)?;
        spotify.add_to_pipeline(&pipeline)?;

        livestream.link_elements()?;
        // music.link_elements()?;
        spotify.link_elements()?;
        common.link_elements()?;

        livestream
            .cap_filter
            .link(&common.audio_mixer)
            .expect("Failed to link livestream to audio mixer");

        spotify.set_active_appsrc().expect("Failed");
        spotify.input_selector.link(&common.audio_mixer).expect("Failed to link audio mixer");
        Self::connect_dynamic_pads(&livestream)?;

        Ok(Self {
            pipeline,
            livestream,
            spotify,
            // music,
            common,
        })
    }

    fn connect_dynamic_pads(livestream: &LivestreamElements) -> Result<(), Error> {
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
impl SpotifyElements {
    pub fn new() -> Result<Self, Error> {
        // Create the appsrc element
        let app_source = ElementFactory::make_with_name("appsrc", Some("spotify_app_source"))
            .expect("Could not create appsrc element");

        // Create the audioconvert element
        let audio_convert =
            ElementFactory::make_with_name("audioconvert", Some("spotify_audio_convert"))
                .expect("Could not create audioconvert element");

        // Create the audioresample element
        let audio_resample =
            ElementFactory::make_with_name("audioresample", Some("spotify_audio_resample"))
                .expect("Could not create audioresample element");

        // Create an input-selector element.
        let input_selector =
            ElementFactory::make_with_name("input-selector", Some("spotify_selector"))
                .expect("Could not create input-selector element");

        // Create a silent source (audiotestsrc) set to produce silence.
        let silent_src = ElementFactory::make_with_name("audiotestsrc", Some("spotify_silent_src"))
            .expect("Could not create audiotestsrc element");
        silent_src.set_property_from_str("wave", &"silence");

        // Set up properties on appsrc.
        let caps = Caps::builder("audio/x-raw")
            .field("format", &"F64LE")
            .field("channels", &2)
            .field("rate", &44100)
            .field("layout", &"interleaved")
            .build();

        app_source.set_property("caps", &caps);
        app_source.set_property("is-live", &true);
        app_source.set_property("format", &gstreamer::Format::Time);
        app_source.set_property("max-bytes", &1024u64);
        app_source.set_property("block", &true);

        Ok(Self {
            app_source,
            audio_convert,
            audio_resample,
            input_selector,
            silent_src,
        })
    }

    pub fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        // Add all branch elements to the pipeline.
        pipeline.add_many(&[
            &self.app_source,
            &self.audio_convert,
            &self.audio_resample,
            &self.input_selector,
            &self.silent_src,
        ])?;
        Ok(())
    }

    pub fn link_elements(&self) -> Result<(), Error> {
        // Link the appsrc branch: appsrc → audioconvert → audioresample.
        gstreamer::Element::link_many(&[
            &self.app_source,
            &self.audio_convert,
            &self.audio_resample,
        ])
        .expect("Failed to link appsrc branch");

        // The input-selector element has request sink pads (named "sink_%u").
        // Request one sink pad for the appsrc branch...
        let app_sink_pad = self
            .input_selector
            .request_pad_simple("sink_%u")
            .expect("Failed to get input-selector sink pad for appsrc branch");
        // ...and one for the silent source.
        let silent_sink_pad = self
            .input_selector
            .request_pad_simple("sink_%u")
            .expect("Failed to get input-selector sink pad for silent branch");

        // Link the output of the real-data branch (audio_resample's src pad)
        // to the requested sink pad for the appsrc branch.
        self.audio_resample
            .link_pads(Some("src"), &self.input_selector, Some(&*app_sink_pad.name()))
            .expect("Failed to link appsrc branch to input-selector");

        // Link the silent source to the input-selector.
        // (audiotestsrc has a fixed src pad so a normal link works)
        self.silent_src
            .link(&self.input_selector)
            .expect("Failed to link silent source to input-selector");

        // Optionally, you can set the active pad.
        // For example, default to the appsrc branch:
        self.input_selector
            .set_property("active-pad", &app_sink_pad);

        // The input-selector's src pad will be linked to the mixer in your main pipeline.
        Ok(())
    }

    // Optionally, add helper methods to switch the active pad when needed.
    pub fn set_active_appsrc(&self) -> Result<(), Error> {
        // This method should be called when your appsrc branch is producing data.
        let pads = self.input_selector.pads();
        for pad in pads {
            if pad.name().contains("sink") && pad.name().contains("0") {
                self.input_selector.set_property("active-pad", &pad);
                break;
            }
        }
        Ok(())
    }

    pub fn set_active_silent(&self) -> Result<(), Error> {
        // This method can be called when you detect that the appsrc branch has stalled.
        let pads = self.input_selector.pads();
        for pad in pads {
            if pad.name().contains("sink") && pad.name().contains("1") {
                self.input_selector.set_property("active-pad", &pad);
                break;
            }
        }
        Ok(())
    }
}

impl LivestreamElements {
    fn new() -> Result<Self, Error> {
        let source = ElementFactory::make_with_name("rtspsrc", Some("livestream_source"))
            .expect("Could not create livestream_source element.");
        let depay = ElementFactory::make_with_name("rtpmp4gdepay", Some("livestream_depay"))
            .expect("Could not create livestream_depay element.");
        let parse = ElementFactory::make_with_name("aacparse", Some("livestream_parse"))
            .expect("Could not create livestream_parse element.");
        let decoder = ElementFactory::make_with_name("decodebin", Some("livestream_decoder"))
            .expect("Could not create livestream_decoder element.");
        let queue = ElementFactory::make_with_name("queue", Some("livestream_queue"))
            .expect("Could not create livestream_queue element.");
        let convert = ElementFactory::make_with_name("audioconvert", Some("livestream_convert"))
            .expect("Could not create livestream_convert element.");
        let resample = ElementFactory::make_with_name("audioresample", Some("livestream_resample"))
            .expect("Could not create livestream_resample element.");
        let buffer = ElementFactory::make_with_name("queue", Some("livestream_buffer"))
            .expect("Could not create livestream_buffer element.");
        let rgvolume = ElementFactory::make_with_name("rgvolume", Some("livestream_rgvolume"))
            .expect("Could not create livestream_rgvolume element.");
        // let dsp = ElementFactory::make_with_name("webrtcdsp", Some("livestream_dsp"))
        //     .expect("Could not create livestream_dsp element.");
        let cap_filter =
            ElementFactory::make_with_name("capsfilter", Some("livestream_cap_filter"))
                .expect("Failed to create capsfilter");
        let cap_resample =
            ElementFactory::make_with_name("audioresample", Some("livestream_cap_resample"))
                .expect("Could not create audioresample element for capsfilter");
        let cap_convert =
            ElementFactory::make_with_name("audioconvert", Some("capsfilter_converter"))
                .expect("Could not create audioconvert element for capsfilter");

        // Set properties
        source.set_property("location", &"rtsp://10.0.0.12:8554/camera.rlc_520a_clear");
        source.set_property("protocols", RTSPLowerTrans::TCP);

        // dsp.set_property("echo-cancel", &false);
        // dsp.set_property("noise-suppression", &true);
        // dsp.set_property_from_str("noise-suppression-level", "very-high");
        // dsp.set_property("voice-detection", &true);
        // dsp.set_property("extended-filter", &true);

        // Reduce volume
        // rgvolume.set_property("pre-amp", &-30.0f64);

        buffer.set_property("max-size-buffers", &0u32);
        buffer.set_property("max-size-bytes", &0u32);
        buffer.set_property("max-size-time", &(900_000_000u64));

        cap_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw")
                .field("format", &"S16LE")
                .field("rate", &44100)
                .field("channels", &2)
                .build(),
        );

        Ok(Self {
            source,
            depay,
            parse,
            decoder,
            queue,
            convert,
            resample,
            buffer,
            volume: rgvolume,
            // dsp,
            cap_filter,
            cap_resample,
            cap_convert,
        })
    }

    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        pipeline.add_many(&[
            &self.source,
            &self.depay,
            &self.parse,
            &self.decoder,
            &self.queue,
            &self.convert,
            &self.resample,
            &self.volume,
            // &self.dsp,
            &self.buffer,
            &self.cap_convert,
            &self.cap_resample,
            &self.cap_filter,
        ])?;
        Ok(())
    }

    fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[&self.depay, &self.parse, &self.decoder])?;
        Element::link_many(&[
            &self.queue,
            &self.convert,
            &self.resample,
            &self.volume,
            // &self.dsp,
            &self.buffer,
            &self.cap_convert,
            &self.cap_resample,
            &self.cap_filter,
        ])?;
        Ok(())
    }
}

pub struct MusicElements {
    source: Element,
    pub(crate) volume: Element,
    cap_filter: Element,
    buffer: Element,
}

pub struct LivestreamElements {
    source: Element,
    depay: Element,
    parse: Element,
    decoder: Element,
    queue: Element,
    convert: Element,
    resample: Element,
    buffer: Element,
    volume: Element,
    // dsp: Element,
    cap_filter: Element,
    cap_resample: Element,
    cap_convert: Element,
}

impl MusicElements {
    fn new() -> Result<Self, Error> {
        let source = ElementFactory::make_with_name("pulsesrc", Some("music_source"))
            .expect("Could not create music_source element.");
        let volume = ElementFactory::make_with_name("volume", Some("livestream_volume"))
            .expect("Could not create livestream_volume element.");
        let cap_filter = ElementFactory::make_with_name("capsfilter", Some("music_cap_filter"))
            .expect("Failed to create music_cap_filter");
        let buffer = ElementFactory::make_with_name("queue", Some("music_buffer"))
            .expect("Could not create livestream_buffer element.");

        // Set properties
        source.set_property(
            "device",
            &"alsa_output.platform-bcm2835_audio.analog-stereo.monitor",
        );
        volume.set_property("volume", 1.0f64);
        cap_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw")
                .field("format", &"S16LE")
                .field("rate", &44100)
                .field("channels", &2)
                .build(),
        );

        buffer.set_property("max-size-buffers", &0u32);
        buffer.set_property("max-size-bytes", &0u32);
        buffer.set_property("max-size-time", &(750_000_000u64));

        Ok(Self {
            source,
            volume,
            cap_filter,
            buffer,
        })
    }

    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        pipeline.add_many(&[&self.source, &self.cap_filter, &self.buffer, &self.volume])?;
        Ok(())
    }

    fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[&self.source, &self.cap_filter, &self.buffer, &self.volume])?;
        Ok(())
    }
}

pub struct CommonElements {
    audio_mixer: Element,
    aac_encoder: Element,
    stereo_filter: Element,
    mp4_mux: Element,
    rtsp_sink: Element,
}

impl CommonElements {
    fn new() -> Result<Self, Error> {
        let audio_mixer = ElementFactory::make_with_name("audiomixer", Some("AudioMixer"))
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

        // mp3_encoder.set_property("bitrate", &320);
        rtsp_sink.set_property("location", &"rtsp://10.0.0.21:8554/sleep");
        stereo_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw").field("channels", &2).build(),
        );

        Ok(Self {
            audio_mixer,
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
            &self.aac_encoder,
            &self.rtsp_sink,
        ])?;
        Ok(())
    }
}
