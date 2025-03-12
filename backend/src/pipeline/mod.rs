mod livestream;
pub mod main;
pub mod spotify;

pub use crate::pipeline::main::MainPipeline;
use gstreamer::prelude::{
    Cast, ElementExt, ElementExtManual, GObjectExtManualGst, GstBinExtManual, GstObjectExt,
    ObjectExt, PadExt, PipelineExt,
};
