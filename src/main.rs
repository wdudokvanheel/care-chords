use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_rtsp::RTSPLowerTrans;
use std::sync::{Arc, Mutex};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize GStreamer
    gst::init()?;

    // Create the pipeline
    let pipeline = gst::Pipeline::new();

    // Create elements for the first RTSP source (camera)
    let rtspsrc1 = gst::ElementFactory::make_with_name("rtspsrc", Some("rtspsrc1"))
        .expect("Could not create rtspsrc1 element.");
    let rtpmp4gdepay1 = gst::ElementFactory::make_with_name("rtpmp4gdepay", Some("rtpmp4gdepay1"))
        .expect("Could not create rtpmp4gdepay1 element.");
    let aacparse1 = gst::ElementFactory::make_with_name("aacparse", Some("aacparse1"))
        .expect("Could not create aacparse1 element.");
    let decodebin1 = gst::ElementFactory::make_with_name("decodebin", Some("decodebin1"))
        .expect("Could not create decodebin1 element.");
    let queue1 = gst::ElementFactory::make_with_name("queue", Some("queue1"))
        .expect("Could not create queue1 element.");
    let audioconvert1 = gst::ElementFactory::make_with_name("audioconvert", Some("audioconvert1"))
        .expect("Could not create audioconvert1 element.");
    let audioresample1 = gst::ElementFactory::make_with_name("audioresample", Some("audioresample1"))
        .expect("Could not create audioresample1 element.");
    let buffer1 = gst::ElementFactory::make_with_name("queue", Some("buffer1"))
        .expect("Could not create buffer1 element.");

    // Create webrtcdsp element
    let webrtcdsp1 = gst::ElementFactory::make_with_name("webrtcdsp", Some("webrtcdsp1"))
        .expect("Could not create webrtcdsp1 element.");
    webrtcdsp1.set_property("echo-cancel", &false);
    webrtcdsp1.set_property("noise-suppression", &true);
    webrtcdsp1.set_property_from_str("noise-suppression-level", &"high");
    webrtcdsp1.set_property("voice-detection", &true);
    webrtcdsp1.set_property("extended-filter", &true);

    let volume1 = gst::ElementFactory::make_with_name("volume", Some("volume1")).expect("Could not create volume element.");
    volume1.set_property("volume", 0.1f64);

    // Create elements for the second RTSP source (Spotify)
    let rtspsrc2 = gst::ElementFactory::make_with_name("rtspsrc", Some("rtspsrc2"))
        .expect("Could not create rtspsrc2 element.");
    let rtpmp4gdepay2 = gst::ElementFactory::make_with_name("rtpmp4gdepay", Some("rtpmp4gdepay2"))
        .expect("Could not create rtpmp4gdepay2 element.");
    let aacparse2 = gst::ElementFactory::make_with_name("aacparse", Some("aacparse2"))
        .expect("Could not create aacparse2 element.");
    let decodebin2 = gst::ElementFactory::make_with_name("decodebin", Some("decodebin2"))
        .expect("Could not create decodebin2 element.");
    let queue2 = gst::ElementFactory::make_with_name("queue", Some("queue2"))
        .expect("Could not create queue2 element.");
    let audioconvert2 = gst::ElementFactory::make_with_name("audioconvert", Some("audioconvert2"))
        .expect("Could not create audioconvert2 element.");
    let audioresample2 = gst::ElementFactory::make_with_name("audioresample", Some("audioresample2"))
        .expect("Could not create audioresample2 element.");

    // Common elements
    let audiomixer = gst::ElementFactory::make_with_name("audiomixer", Some("audiomixer"))
        .expect("Could not create audiomixer element.");
    let lamemp3enc = gst::ElementFactory::make_with_name("lamemp3enc", Some("lamemp3enc"))
        .expect("Could not create lamemp3enc element.");
    let flacenc = gst::ElementFactory::make_with_name("flacenc", Some("flacenc"))
        .expect("Could not create flacenc element.");
    let rtspclientsink = gst::ElementFactory::make_with_name("rtspclientsink", Some("rtspclientsink"))
        .expect("Could not create rtspclientsink element.");

    // Create elements for forcing stereo output after mixing
    let audiostereo = gst::ElementFactory::make_with_name("capsfilter", Some("audiostereo"))
        .expect("Could not create audiostereo element.");
    audiostereo.set_property("caps", &gst::Caps::builder("audio/x-raw").field("channels", &2).build());



    // Set element properties
    rtspsrc1.set_property("location", &"rtsp://10.0.0.12:8554/camera.rlc_520a_clear");
    rtspsrc1.set_property("protocols", RTSPLowerTrans::TCP);
    rtspsrc2.set_property("location", &"rtsp://10.0.0.153:8554/spotify");
    rtspsrc2.set_property("protocols", RTSPLowerTrans::TCP);
    rtspclientsink.set_property("location", &"rtsp://localhost:8554/lumi");
    lamemp3enc.set_property("bitrate", &320);

    buffer1.set_property("max-size-buffers", &0u32);
    buffer1.set_property("max-size-bytes", &0u32);
    buffer1.set_property("max-size-time", &(500_000_000u64));

    // Add elements to the pipeline
    pipeline.add_many(&[
        &rtspsrc1, &rtpmp4gdepay1, &aacparse1, &decodebin1, &queue1, &audioconvert1, &audioresample1, &webrtcdsp1, &buffer1,
        &rtspsrc2, &rtpmp4gdepay2, &aacparse2, &decodebin2, &queue2, &audioconvert2, &audioresample2, &volume1,
        &audiomixer, &audiostereo, &lamemp3enc, &rtspclientsink
    ])?;

    // Link static elements for the first RTSP source
    gst::Element::link_many(&[
        &rtpmp4gdepay1, &aacparse1, &decodebin1
    ])?;
    gst::Element::link_many(&[
        &queue1, &audioconvert1, &audioresample1, &webrtcdsp1, &buffer1, &audiomixer
    ])?;

    // Link static elements for the second RTSP source
    gst::Element::link_many(&[
        &rtpmp4gdepay2, &aacparse2, &decodebin2
    ])?;
    gst::Element::link_many(&[
        &queue2, &audioconvert2, &audioresample2, &volume1, &audiomixer
    ])?;

    gst::Element::link_many(&[
        &audiomixer, &audiostereo, &lamemp3enc, &rtspclientsink
    ])?;

    // Connect to the pad-added signal of the rtspsrc1 element to dynamically link its source pad
    rtspsrc1.connect_pad_added(move |_src, src_pad| {
        let src_pad_caps = src_pad.current_caps().unwrap();
        let src_pad_structure = src_pad_caps.structure(0).unwrap();

        if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
            if media_type == "audio" {
                let sink_pad = rtpmp4gdepay1.static_pad("sink").expect("Failed to get sink pad");
                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link rtspsrc1 audio: {}", err);
                }
            }
        }
    });

    // Connect to the pad-added signal of the rtspsrc2 element to dynamically link its source pad
    rtspsrc2.connect_pad_added(move |_src, src_pad| {
        let src_pad_caps = src_pad.current_caps().unwrap();
        let src_pad_structure = src_pad_caps.structure(0).unwrap();

        if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
            if media_type == "audio" {
                let sink_pad = rtpmp4gdepay2.static_pad("sink").expect("Failed to get sink pad");
                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link rtspsrc2 audio: {}", err);
                }
            }
        }
    });

    decodebin1.connect_pad_added(move |_, src_pad| {
        let queue1_sink_pad = queue1.static_pad("sink").expect("Failed to get sink pad from queue1");

        if let Err(err) = src_pad.link(&queue1_sink_pad) {
            eprintln!("Failed to link decodebin to queue1: {}", err);
        }
    });

    decodebin2.connect_pad_added(move |_, src_pad| {
        let queue1_sink_pad = queue2.static_pad("sink").expect("Failed to get sink pad from queue1");

        if let Err(err) = src_pad.link(&queue1_sink_pad) {
            eprintln!("Failed to link decodebin to queue1: {}", err);
        }
    });


    // Set up the pipeline
    pipeline.set_state(gst::State::Playing)?;

    // Use a Tokio task to manage the GStreamer bus messages asynchronously
    let bus = pipeline.bus().unwrap();
    let pipeline_clone = pipeline.clone();
    tokio::spawn(async move {
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("End of stream reached");
                    break;
                }
                gst::MessageView::Error(err) => {
                    eprintln!("Error from {}: {}", err.src().unwrap().path_string(), err.error());
                    break;
                }
                _ => (),
            }
        }

        // Clean up the pipeline
        pipeline_clone.set_state(gst::State::Null).unwrap();
    });

    let volume1 = Arc::new(Mutex::new(volume1));
    let volume1_clone = Arc::clone(&volume1);

    // tokio::spawn(async move {
    //     let mut volume_level = 0.0;
    //     let mut increasing = true;
    //     let mut interval = time::interval(Duration::from_secs(1));
    //
    //     loop {
    //         interval.tick().await;
    //
    //         let mut volume1 = volume1_clone.lock().unwrap();
    //         volume1.set_property("volume", volume_level);
    //
    //         if increasing {
    //             volume_level += 0.1;
    //             if volume_level >= 1.0 {
    //                 increasing = false;
    //             }
    //         } else {
    //             volume_level -= 0.1;
    //             if volume_level <= 0.0 {
    //                 increasing = true;
    //             }
    //         }
    //     }
    // });

    // Keep the runtime alive (you could also use some other async logic here)
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");

    Ok(())
}
