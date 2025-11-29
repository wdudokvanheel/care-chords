struct Playlist: Identifiable {
    let id = UUID()
    let name: String
    let uri: String
    let image: URL?
    let folder: String?

    init(_ name: String, _ uri: String, _ image: URL? = nil, folder: String? = nil) {
        self.name = name
        self.uri = uri
        self.image = image
        self.folder = folder
    }
}
