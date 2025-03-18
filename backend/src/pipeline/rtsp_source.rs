use anyhow::Error;
use gstreamer::prelude::{
    ElementExt, GObjectExtManualGst, GstBinExt, GstBinExtManual, ObjectExt, PadExt,
};
use gstreamer::{Caps, Element, ElementFactory, Pipeline};
use gstreamer_rtsp::RTSPLowerTrans;
use log::error;

pub struct RTSPSourcePipeline {
    pub source: Element,
    pub depay: Element,
    parse: Element,
    pub decoder: Element,
    pre_mix_convert: Element,
    resample_mixer: Element,
    pub pre_mix_queue: Element,
    post_mix_convert: Element,
    post_mix_resample: Element,
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
        let resample_mixer =
            ElementFactory::make_with_name("audiomixer", Some("livestream_resample_mixer"))
                .expect("Could not create livestream_queue element.");
        let pre_mix_queue = ElementFactory::make_with_name("queue2", Some("livestream_queue"))
            .expect("Could not create livestream_queue element.");
        let pre_mix_convert =
            ElementFactory::make_with_name("audioconvert", Some("livestream_convert"))
                .expect("Could not create livestream_convert element.");
        let post_mix_convert =
            ElementFactory::make_with_name("audioconvert", Some("livestream_convert2"))
                .expect("Could not create livestream_convert element.");
        let post_mix_resample =
            ElementFactory::make_with_name("audioresample", Some("livestream_resample"))
                .expect("Could not create livestream_resample element.");

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

        cap_filter.set_property(
            "caps",
            &Caps::builder("audio/x-raw")
                .field("format", &"F32LE")
                .field("rate", &44100)
                .field("channels", &2)
                .build(),
        );

        Ok(Self {
            source,
            depay,
            parse,
            decoder,
            pre_mix_convert,
            pre_mix_queue,
            resample_mixer,
            post_mix_convert,
            post_mix_resample,
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
            &self.pre_mix_convert,
            &self.pre_mix_queue,
            &self.resample_mixer,
            &self.post_mix_convert,
            &self.post_mix_resample,
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
            &self.pre_mix_convert,
            &self.pre_mix_queue,
            &self.resample_mixer,
        ])?;

        let mut post_mix_chain = vec![
            &self.resample_mixer,
            &self.post_mix_convert,
            &self.post_mix_resample,
            &self.cap_filter,
        ];

        if let Some(dsp) = &self.dsp {
            post_mix_chain.insert(1, dsp);
        }

        Element::link_many(&post_mix_chain)?;

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

        let queue_clone = self.pre_mix_convert.clone();
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
