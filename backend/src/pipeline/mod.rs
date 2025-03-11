mod livestream;
pub mod main;
mod spotify;

pub use crate::pipeline::main::MainPipeline;
use crate::pipeline::spotify::SpotifyInputSourceSelector;
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


