use anyhow::Error;
use gstreamer::prelude::*;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{RTSPMediaFactory, RTSPServer};

pub struct RtspServer {
    server: RTSPServer,
    port: u16,
}

impl RtspServer {
    pub fn new(port: u16) -> Result<Self, Error> {
        let server = RTSPServer::new();
        server.set_service(&port.to_string());

        let factory = RTSPMediaFactory::new();
        factory.set_shared(true);
        
        // We receive audio via UDP from the main pipeline and payload it for RTSP
        // The udpsrc port must match the udpsink port in audio_pipeline.rs
        let pipeline_str = "udpsrc port=5000 ! application/x-rtp,media=audio,clock-rate=44100,encoding-name=L16,channels=2,payload=96 ! rtpL16depay ! audioconvert ! avenc_aac ! rtpmp4apay name=pay0 pt=96";
        
        factory.set_launch(pipeline_str);

        let mounts = server.mount_points().ok_or_else(|| anyhow::anyhow!("Could not get mount points"))?;
        
        factory.connect_media_configure(|_factory, media| {
            log::info!("RTSP Media configured for stream");
            media.connect_prepared(|_media| {
                log::info!("RTSP Media prepared");
            });
            media.connect_unprepared(|_media| {
                log::info!("RTSP Media unprepared");
            });
        });

        mounts.add_factory("/sleep", factory);

        server.connect_client_connected(|_server, _client| {
            log::info!("RTSP Client connected");
        });

        Ok(Self { server, port })
    }

    pub async fn start(&self) -> Result<(), Error> {
        let server = self.server.clone();
        let port = self.port;
        
        std::thread::spawn(move || {
            let main_loop = gstreamer::glib::MainLoop::new(None, false);
            let context = main_loop.context();
            server.attach(Some(&context)).expect("Failed to attach server");
            
            log::info!("RTSP server listening on port {}", port);
            
            main_loop.run();
        });

        Ok(())
    }
}
