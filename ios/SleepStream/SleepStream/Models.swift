struct Playlist: Identifiable {
    let id = UUID()
    let name: String
    let uri: String
    let image: URL?

    init(_ name: String, _ uri: String, _ image: URL? = nil) {
        self.name = name
        self.uri = uri
        self.image = image
    }
}
