@objc protocol GStreamerBackendDelegate {
    func gStreamerInitialized()
    func gstreamerMessage(message: String)
}

@objc protocol GStreamerAudioBackendDelegate: GStreamerBackendDelegate {
    func gstreamerAudioState(state: AudioState)
}

@objc protocol GStreamerVideoBackendDelegate: GStreamerBackendDelegate {
    func gstreamerDidReceiveVideoResolution(width: Int, height: Int)
}
