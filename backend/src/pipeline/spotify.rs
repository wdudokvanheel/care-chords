use anyhow::Error;
use gstreamer::prelude::{
    ElementExtManual, GObjectExtManualGst, GstBinExtManual, GstObjectExt, ObjectExt,
};
use gstreamer::{Caps, Element, ElementFactory, Pipeline};
use std::sync::{Arc, Mutex};

pub enum SpotifyInputSourceSelector {
    Silence,
    Spotify,
}

pub struct SpotifyPipeline {
    pub input_source: Arc<Mutex<SpotifyInputSourceSelector>>,
    pub app_source: Element,
    pub queue: Element,
    audio_convert: Element,
    audio_resample: Element,
    silent_src: Element,
    pub input_selector: Element,
}

impl SpotifyPipeline {
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

        // Create a silent source (audiotestsrc) to produce silence.
        let silent_src = ElementFactory::make_with_name("audiotestsrc", Some("spotify_silent_src"))
            .expect("Could not create audiotestsrc element");
        silent_src.set_property_from_str("wave", &"silence");

        let queue = ElementFactory::make_with_name("queue", Some("spotify_queue"))
            .expect("Could not create livestream_queue element.");

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
        app_source.set_property("max-bytes", &10_000u64);
        app_source.set_property("do-timestamp", &true);
        app_source.set_property("block", &true);

        queue.set_property("max-size-bytes", &5_0000u32);

        Ok(Self {
            app_source,
            audio_convert,
            audio_resample,
            input_selector,
            silent_src,
            queue,
            input_source: Arc::new(Mutex::new(SpotifyInputSourceSelector::Silence)),
        })
    }

    pub fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        // Add all branch elements to the pipeline.
        pipeline.add_many(&[
            &self.app_source,
            &self.queue,
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
            &self.queue,
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
            .link_pads(
                Some("src"),
                &self.input_selector,
                Some(&*app_sink_pad.name()),
            )
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
