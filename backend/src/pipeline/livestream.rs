use anyhow::Error;
use gstreamer::prelude::{GstBinExtManual, ObjectExt};
use gstreamer::{Caps, Element, ElementFactory, Pipeline};
use gstreamer_rtsp::RTSPLowerTrans;

pub struct LivestreamPipeline {
    pub source: Element,
    pub depay: Element,
    parse: Element,
    pub decoder: Element,
    pub queue: Element,
    convert: Element,
    resample: Element,
    buffer: Element,
    volume: Element,
    // dsp: Element,
    pub cap_filter: Element,
    cap_resample: Element,
    cap_convert: Element,
}

impl LivestreamPipeline {
    pub fn new() -> Result<Self, Error> {
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

        // buffer.set_property("max-size-buffers", &0u32);
        // buffer.set_property("max-size-bytes", &10000_000u32);
        // queue.set_property("max-size-bytes", &10000_000u32);
        // buffer.set_property("max-size-time", &(9000_000_000u64));

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

    pub fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
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

    pub fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[&self.depay, &self.parse, &self.decoder])?;
        Element::link_many(&[
            &self.queue,
            &self.buffer,
            &self.convert,
            &self.resample,
            &self.volume,
            // &self.dsp,
            &self.cap_convert,
            &self.cap_resample,
            &self.cap_filter,
        ])?;
        Ok(())
    }
}
