import Foundation

struct AudioItem: Identifiable {
    let id: String
    let name: String
    let reference: String
    let source: String
    let kind: AudioItemKind
    let image: URL?
    let folder: String?

    init(
        id: String,
        name: String,
        reference: String,
        source: String,
        kind: AudioItemKind,
        image: URL? = nil,
        folder: String? = nil
    ) {
        self.id = id
        self.name = name
        self.reference = reference
        self.source = source
        self.kind = kind
        self.image = image
        self.folder = folder
    }
}

enum AudioItemKind: String {
    case file
    case folder
    case playlist
}

typealias Playlist = AudioItem
