@objc protocol GStreamerBackendDelegate {
    func gStreamerInitialized()
    func gstreamerMessage(message: String)
    func gstreamerAudioState(state: AudioState)
}
