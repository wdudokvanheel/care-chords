use std::time::Duration;
use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_rtsp::RTSPLowerTrans;
use std::sync::{Arc, Mutex};
use tokio;
use tokio::time;
use warp::http::StatusCode;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize GStreamer
    gst::init()?;

    // Create the pipeline
    let pipeline = gst::Pipeline::new();

    // Create elements for the livestream source
    let livestream_source = gst::ElementFactory::make_with_name("rtspsrc", Some("livestream_source"))
        .expect("Could not create livestream_source element.");
    let livestream_depay = gst::ElementFactory::make_with_name("rtpmp4gdepay", Some("livestream_depay"))
        .expect("Could not create livestream_depay element.");
    let livestream_parse = gst::ElementFactory::make_with_name("aacparse", Some("livestream_parse"))
        .expect("Could not create livestream_parse element.");
    let livestream_decoder = gst::ElementFactory::make_with_name("decodebin", Some("livestream_decoder"))
        .expect("Could not create livestream_decoder element.");
    let livestream_queue = gst::ElementFactory::make_with_name("queue", Some("livestream_queue"))
        .expect("Could not create livestream_queue element.");
    let livestream_convert = gst::ElementFactory::make_with_name("audioconvert", Some("livestream_convert"))
        .expect("Could not create livestream_convert element.");
    let livestream_resample = gst::ElementFactory::make_with_name("audioresample", Some("livestream_resample"))
        .expect("Could not create livestream_resample element.");
    let livestream_buffer = gst::ElementFactory::make_with_name("queue", Some("livestream_buffer"))
        .expect("Could not create livestream_buffer element.");

    // Create webrtcdsp element
    let livestream_dsp = gst::ElementFactory::make_with_name("webrtcdsp", Some("livestream_dsp"))
        .expect("Could not create livestream_dsp element.");
    livestream_dsp.set_property("echo-cancel", &false);
    livestream_dsp.set_property("noise-suppression", &true);
    livestream_dsp.set_property_from_str("noise-suppression-level", &"high");
    livestream_dsp.set_property("voice-detection", &true);
    livestream_dsp.set_property("extended-filter", &true);

    let livestream_volume = gst::ElementFactory::make_with_name("volume", Some("livestream_volume"))
        .expect("Could not create livestream_volume element.");
    livestream_volume.set_property("volume", 1.0f64);

    // Create elements for the music source
    let music_source = gst::ElementFactory::make_with_name("rtspsrc", Some("music_source"))
        .expect("Could not create music_source element.");
    let music_depay = gst::ElementFactory::make_with_name("rtpmp4gdepay", Some("music_depay"))
        .expect("Could not create music_depay element.");
    let music_parse = gst::ElementFactory::make_with_name("aacparse", Some("music_parse"))
        .expect("Could not create music_parse element.");
    let music_decoder = gst::ElementFactory::make_with_name("decodebin", Some("music_decoder"))
        .expect("Could not create music_decoder element.");
    let music_queue = gst::ElementFactory::make_with_name("queue", Some("music_queue"))
        .expect("Could not create music_queue element.");
    let music_convert = gst::ElementFactory::make_with_name("audioconvert", Some("music_convert"))
        .expect("Could not create music_convert element.");
    let music_resample = gst::ElementFactory::make_with_name("audioresample", Some("music_resample"))
        .expect("Could not create music_resample element.");

    // Common elements
    let audio_mixer = gst::ElementFactory::make_with_name("audiomixer", Some("audio_mixer"))
        .expect("Could not create audio_mixer element.");
    let mp3_encoder = gst::ElementFactory::make_with_name("lamemp3enc", Some("mp3_encoder"))
        .expect("Could not create mp3_encoder element.");
    let flac_encoder = gst::ElementFactory::make_with_name("flacenc", Some("flac_encoder"))
        .expect("Could not create flac_encoder element.");
    let rtsp_sink = gst::ElementFactory::make_with_name("rtspclientsink", Some("rtsp_sink"))
        .expect("Could not create rtsp_sink element.");

    // Create elements for forcing stereo output after mixing
    let stereo_filter = gst::ElementFactory::make_with_name("capsfilter", Some("stereo_filter"))
        .expect("Could not create stereo_filter element.");
    stereo_filter.set_property("caps", &gst::Caps::builder("audio/x-raw").field("channels", &2).build());

    // Set element properties
    livestream_source.set_property("location", &"rtsp://10.0.0.12:8554/camera.rlc_520a_clear");
    livestream_source.set_property("protocols", RTSPLowerTrans::TCP);
    music_source.set_property("location", &"rtsp://10.0.0.153:8554/spotify");
    music_source.set_property("protocols", RTSPLowerTrans::TCP);
    rtsp_sink.set_property("location", &"rtsp://localhost:8554/sleep");
    mp3_encoder.set_property("bitrate", &320);

    livestream_buffer.set_property("max-size-buffers", &0u32);
    livestream_buffer.set_property("max-size-bytes", &0u32);
    livestream_buffer.set_property("max-size-time", &(500_000_000u64));

    // Add elements to the pipeline
    pipeline.add_many(&[
        &livestream_source, &livestream_depay, &livestream_parse, &livestream_decoder, &livestream_queue, &livestream_convert, &livestream_resample, &livestream_dsp, &livestream_buffer,
        &music_source, &music_depay, &music_parse, &music_decoder, &music_queue, &music_convert, &music_resample, &livestream_volume,
        &audio_mixer, &stereo_filter, &mp3_encoder, &rtsp_sink
    ])?;

    // Link static elements for the livestream source
    gst::Element::link_many(&[
        &livestream_depay, &livestream_parse, &livestream_decoder
    ])?;
    gst::Element::link_many(&[
        &livestream_queue, &livestream_convert, &livestream_resample, &livestream_dsp, &livestream_buffer, &audio_mixer
    ])?;

    // Link static elements for the music source
    gst::Element::link_many(&[
        &music_depay, &music_parse, &music_decoder
    ])?;
    gst::Element::link_many(&[
        &music_queue, &music_convert, &music_resample, &livestream_volume, &audio_mixer
    ])?;

    gst::Element::link_many(&[
        &audio_mixer, &stereo_filter, &mp3_encoder, &rtsp_sink
    ])?;

    // Connect to the pad-added signal of the livestream_source element to dynamically link its source pad
    livestream_source.connect_pad_added(move |_src, src_pad| {
        let src_pad_caps = src_pad.current_caps().unwrap();
        let src_pad_structure = src_pad_caps.structure(0).unwrap();

        if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
            if media_type == "audio" {
                let sink_pad = livestream_depay.static_pad("sink").expect("Failed to get sink pad");
                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link livestream_source audio: {}", err);
                }
            }
        }
    });

    // Connect to the pad-added signal of the music_source element to dynamically link its source pad
    music_source.connect_pad_added(move |_src, src_pad| {
        let src_pad_caps = src_pad.current_caps().unwrap();
        let src_pad_structure = src_pad_caps.structure(0).unwrap();

        if let Ok(media_type) = src_pad_structure.get::<&str>("media") {
            if media_type == "audio" {
                let sink_pad = music_depay.static_pad("sink").expect("Failed to get sink pad");
                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!("Failed to link music_source audio: {}", err);
                }
            }
        }
    });

    livestream_decoder.connect_pad_added(move |_, src_pad| {
        let livestream_queue_sink_pad = livestream_queue.static_pad("sink").expect("Failed to get sink pad from livestream_queue");

        if let Err(err) = src_pad.link(&livestream_queue_sink_pad) {
            eprintln!("Failed to link livestream_decoder to livestream_queue: {}", err);
        }
    });

    music_decoder.connect_pad_added(move |_, src_pad| {
        let music_queue_sink_pad = music_queue.static_pad("sink").expect("Failed to get sink pad from music_queue");

        if let Err(err) = src_pad.link(&music_queue_sink_pad) {
            eprintln!("Failed to link music_decoder to music_queue: {}", err);
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

    let livestream_volume = Arc::new(Mutex::new(livestream_volume));
    let livestream_volume_clone = Arc::clone(&livestream_volume);
    let control_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |body: serde_json::Value| {
            let livestream_volume_clone = Arc::clone(&livestream_volume_clone);
            async move {
                if let Some(time) = body.get("timer") {
                    if let Some(sleep_timer) = time.as_u64() {
                        // Spawn a new task to handle the timer and volume reduction
                        let livestream_volume_clone = Arc::clone(&livestream_volume_clone);
                        tokio::spawn(async move {
                            println!("Starting sleep timer in {} seconds", sleep_timer);

                            // Wait for the specified timer duration
                            time::sleep(Duration::from_secs(sleep_timer)).await;

                            println!("Starting volume decrease");

                            // Gradually reduce the volume over interval
                            let mut interval = time::interval(Duration::from_millis(500));
                            for step in 0..=100 {
                                let volume_level = 1.0 - (step as f64 * 0.01);
                                {
                                    let mut livestream_volume = livestream_volume_clone.lock().unwrap();
                                    livestream_volume.set_property("volume", volume_level);
                                }
                                interval.tick().await;
                            }
                        });

                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "status": "timer started" })),
                            StatusCode::OK,
                        ));
                    }
                }
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
                    StatusCode::BAD_REQUEST,
                ))
            }
        });
    let routes = control_route;

    tokio::spawn(async move {
        println!("Starting server @ :7755");
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });

    // // Keep the runtime alive (you could also use some other async logic here)
    // tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");

    Ok(())
}
