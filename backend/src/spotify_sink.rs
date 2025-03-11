use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use std::sync::mpsc::SyncSender;

pub enum SinkEvent {
    Start,
    Stop,
    Packet(Vec<f64>),
}

// Simple sink that pushes the audio packets to a sync channel
pub struct ChannelSink {
    sender: SyncSender<SinkEvent>,
}

impl ChannelSink {
    pub fn new(sender: SyncSender<SinkEvent>) -> Self {
        ChannelSink { sender }
    }
}

impl Sink for ChannelSink {
    fn start(&mut self) -> SinkResult<()> {
        self.sender.send(SinkEvent::Start).map_err(|e| {
            SinkError::OnWrite("Failed to send audio packet to sync channel".to_string()).into()
        })
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.sender.send(SinkEvent::Stop).map_err(|e| {
            SinkError::OnWrite("Failed to send audio packet to sync channel".to_string()).into()
        })
    }

    fn write(&mut self, packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        match packet {
            AudioPacket::Samples(samples) => {
                return self.sender.send(SinkEvent::Packet(samples)).map_err(|e| {
                    SinkError::OnWrite("Failed to send audio packet to sync channel".to_string())
                        .into()
                });
            }
            _ => {}
        }

        Ok(())
    }
}
