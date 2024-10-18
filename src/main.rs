use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::{ElementExt, GstBinExtManual, GstObjectExt, ObjectExt, PadExt};
use gstreamer_rtsp::RTSPLowerTrans;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Error> {
    // Initialize GStreamer
    gst::init()?;

    // Create the pipeline
    let pipeline = gst::Pipeline::new();

    // Create elements
    let rtspsrc = gst::ElementFactory::make_with_name("rtspsrc", Some("rtspsrc"))
        .expect("Could not create rtspsrc element.");
    let rtpmp4gdepay = gst::ElementFactory::make_with_name("rtpmp4gdepay", Some("rtpmp4gdepay"))
        .expect("Could not create rtpmp4gdepay element.");
    let aacparse = gst::ElementFactory::make_with_name("aacparse", Some("aacparse"))
        .expect("Could not create aacparse element.");
    let queue = gst::ElementFactory::make_with_name("queue", Some("queue"))
        .expect("Could not create queue element.");
    let rtspclientsink = gst::ElementFactory::make_with_name("rtspclientsink", Some("rtspclientsink"))
        .expect("Could not create rtspclientsink element.");

    // Set element properties
    rtspsrc.set_property("location", &"rtsp://10.0.0.12:8554/camera.rlc_520a_clear");
    rtspsrc.set_property("protocols", RTSPLowerTrans::TCP);

    rtspclientsink.set_property("location", &"rtsp://localhost:8554/lumi");

    // Add elements to the pipeline
    pipeline.add_many(&[&rtspsrc, &rtpmp4gdepay, &aacparse, &queue, &rtspclientsink])?;

    // Link elements
    gst::Element::link_many(&[&rtpmp4gdepay, &aacparse, &queue, &rtspclientsink])?;

    // Connect to the pad-added signal of the rtspsrc element to dynamically link its source pad
    rtspsrc.connect_pad_added(move |src, src_pad| {
        let src_pad_caps = src_pad.current_caps().unwrap();
        let src_pad_structure = src_pad_caps.structure(0).unwrap();

        // Get the "media" field from the caps
        if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
            if media_type == "audio" {
                let sink_pad = rtpmp4gdepay.static_pad("sink").expect("Failed to get sink pad");
                let source_name = src_pad.name().to_string();
                let sink_name = sink_pad.name().to_string();

                println!("Connecting audio pads: {} -> {}", source_name, sink_name);

                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link audio pads {} -> {}: {}", source_name, sink_name, err);
                }

                if sink_pad.is_linked() {
                    println!("Audio pad is successfully linked: {}", sink_pad.name());
                }
            } else {
                println!("Ignoring non-audio stream of type: {}", media_type);
            }
        } else {
            println!("Failed to get 'media' field from caps");
        }
    });

    // Start playing the pipeline
    pipeline.set_state(gst::State::Playing)?;

    // Run the pipeline until an error or EOS (End of Stream)
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                eprintln!("Error from {}: {}", err.src().unwrap().path_string(), err.error());
                break;
            }
            _ => (),
        }
    }

    // Clean up
    pipeline.set_state(gst::State::Null)?;

    Ok(())
}

