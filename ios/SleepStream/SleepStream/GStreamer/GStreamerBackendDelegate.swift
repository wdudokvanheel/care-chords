@objc protocol GStreamerAudioBackendDelegate {
    func gStreamerInitialized()
    func gstreamerMessage(message: String)
    func gstreamerAudioState(state: AudioState)
}

@objc protocol GStreamerVideoBackendDelegate {
    func gStreamerInitialized()
    func gstreamerMessage(message: String)
}
