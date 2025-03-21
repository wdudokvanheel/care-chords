use crate::pipeline::audio_pipeline::PipeLineBranch;
use anyhow::Error;
use gstreamer::prelude::{GstBinExtManual, ObjectExt};
use gstreamer::{Caps, Element, ElementFactory, Pipeline};

pub struct SpotifySourcePipeline {
    pub app_source: Element,
    queue: Element,
    convert: Element,
    resample: Element,
}

impl SpotifySourcePipeline {
    pub fn new() -> Result<Self, Error> {
        let app_source = ElementFactory::make_with_name("appsrc", Some("spotify_app_source"))
            .expect("Could not create appsrc element");
        let convert = ElementFactory::make_with_name("audioconvert", Some("spotify_convert"))
            .expect("Could not create audioconvert element");
        let resample = ElementFactory::make_with_name("audioresample", Some("spotify_resample"))
            .expect("Could not create audioresample element");
        let queue = ElementFactory::make_with_name("queue", Some("spotify_queue"))
            .expect("Could not create spotify_queue element.");

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
        app_source.set_property("max-bytes", &500_000u64);
        app_source.set_property("do-timestamp", &true);
        app_source.set_property("block", &true);

        // queue.set_property("max-size-bytes", &100_000u32);

        Ok(Self {
            app_source,
            convert,
            resample,
            queue,
        })
    }
}

impl PipeLineBranch for SpotifySourcePipeline {
    fn add_to_pipeline(&self, pipeline: &Pipeline) -> Result<(), Error> {
        pipeline.add_many(&[&self.app_source, &self.queue, &self.convert, &self.resample])?;
        Ok(())
    }

    fn link_elements(&self) -> Result<(), Error> {
        Element::link_many(&[&self.app_source, &self.queue, &self.convert, &self.resample])?;
        Ok(())
    }

    fn last_element(&self) -> Element {
        self.resample.clone()
    }
}
