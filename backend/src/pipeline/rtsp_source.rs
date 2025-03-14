use anyhow::Error;
use gstreamer::prelude::{ElementExt, GObjectExtManualGst, GstBinExt, GstBinExtManual, ObjectExt, PadExt};
use gstreamer::{Caps, Element, ElementFactory, Pipeline};
use gstreamer_rtsp::RTSPLowerTrans;
use log::error;

pub struct RTSPSourcePipeline {
    pub source: Element,
    pub depay: Element,
    parse: Element,
    pub decoder: Element,
    pub queue: Element,
    convert: Element,
    resample: Element,
    volume: Element,
    dsp: Option<Element>,
    pub cap_filter: Element,
}

impl RTSPSourcePipeline {
    pub fn new(rtsp_url: &str, noise_filter: bool) -> Result<Self, Error> {
        let source = ElementFactory::make_with_name("rtspsrc", Some("livestream_source"))
            .expect("Could not create livestream_source element.");
        let depay = ElementFactory::make_with_name("rtpmp4gdepay", Some("livestream_depay"))
            .expect("Could not create livestream_depay element.");
        let parse = ElementFactory::make_with_name("aacparse", Some("livestream_parse"))
            .expect("Could not create livestream_parse element.");
        let decoder = ElementFactory::make_with_name("decodebin", Some("livestream_decoder"))
            .expect("Could not create livestream_decoder element.");
        let queue = ElementFactory::make_with_name("queue2", Some("livestream_queue"))
            .expect("Could not create livestream_queue element.");
        let convert = ElementFactory::make_with_name("audioconvert", Some("livestream_convert"))
            .expect("Could not create livestream_convert element.");
        let resample = ElementFactory::make_with_name("audioresample", Some("livestream_resample"))
            .expect("Could not create livestream_resample element.");
        let rgvolume = ElementFactory::make_with_name("rgvolume", Some("livestream_rgvolume"))
            .expect("Could not create livestream_rgvolume element.");

        let cap_filter =
            ElementFactory::make_with_name("capsfilter", Some("livestream_cap_filter"))
                .expect("Failed to create capsfilter");

        // Set properties
        source.set_property("location", rtsp_url);
        source.set_property("protocols", RTSPLowerTrans::UDP);
        source.set_property("latency", &50u32);

        let dsp = {
            if noise_filter {
                let dsp = ElementFactory::make_with_name("webrtcdsp", Some("livestream_dsp"))
                    .expect("Could not create livestream_dsp element.");
                dsp.set_property("echo-cancel", &false);
                dsp.set_property("noise-suppression", &true);
                dsp.set_property_from_str("noise-suppression-level", "very-high");
                dsp.set_property("voice-detection", &true);
                dsp.set_property("extended-filter", &true);
                Some(dsp)
            } else {
                None
            }
        };

        // queue.set_property("use-buffering", &true);

        // Reduce volume
        // rgvolume.set_property("pre-amp", &-30.0f64);

        // queue.set_property("max-size-bytes", &10000_000u32);

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
            volume: rgvolume,
            dsp,
            cap_filter,
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
            &self.cap_filter,
        ])?;

        if let Some(dsp) = &self.dsp {
            pipeline.add(dsp)?;
        }
        Ok(())
    }

    pub fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[&self.depay, &self.parse, &self.decoder])?;
        Element::link_many(&[
            &self.queue,
            &self.convert,
            &self.resample,
            &self.volume,
        ])?;

        if let Some(dsp) = &self.dsp {
            Element::link_many(&[&self.volume, &dsp, &self.cap_filter])?;
        }
        else{
            Element::link_many(&[&self.volume, &self.cap_filter])?;
        }

        Ok(())
    }

    pub fn connect_dynamic_pads(&self) -> Result<(), Error> {
        // Clone elements for closure
        let depay_clone = self.depay.clone();
        self.source.connect_pad_added(move |_src, src_pad| {
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

        let queue_clone = self.queue.clone();
        self.decoder.connect_pad_added(move |_, src_pad| {
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
}
