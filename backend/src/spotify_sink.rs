use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use std::sync::mpsc::SyncSender;

// Simple sink that pushes the audio packets to a sync channel
pub struct ChannelSink {
    sender: SyncSender<AudioPacket>,
}

impl ChannelSink {
    pub fn new(sender: SyncSender<AudioPacket>) -> Self {
        ChannelSink { sender }
    }
}

impl Sink for ChannelSink {
    fn start(&mut self) -> SinkResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        self.sender.send(packet).map_err(|e| {
            SinkError::OnWrite("Failed to send audio packet to sync channel".to_string()).into()
        })
    }
}
