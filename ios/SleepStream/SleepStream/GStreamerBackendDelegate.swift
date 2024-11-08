@objc protocol GStreamerBackendDelegate {
    func gStreamerInitialized()
    func gstreamerMessage(message: String)
    func gstreamerAudioState(state: AudioState)
}

@objc enum AudioState: Int {
    case initializing = 0
    case ready = 1
    case paused = 2
    case playing = 3

    public var description: String {
        switch self {
            case .initializing: return "INIT"
            case .ready: return "READY"
            case .paused: return "PAUSE"
            case .playing: return "PLAY"
        }
    }

    public init?(from rawValue: Int) {
        switch rawValue {
            case 0: self = .initializing
            case 1: self = .ready
            case 2: self = .paused
            case 3: self = .playing
            default: return nil
        }
    }
}
