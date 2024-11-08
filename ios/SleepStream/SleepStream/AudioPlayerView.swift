import Combine
import Foundation
import os
import SwiftUI

struct AudioPlayerView: View {
//    @State private var cancellables = Set<AnyCancellable>()

    @StateObject var controller: AudioController = .init()
    @State var playlists: [Playlist] = [
        Playlist("CBL & Rain", "04qC7znZ4eWnTVezaEBOF7"),
        Playlist("Handpan", "0XszLZdqIrit8epvbcEe61"),
        Playlist("Fantasy & Rain", "46ZaYOSrlpvO1qjB1ezofY"),
    ]

    var body: some View {
        VStack {
            Text("Audio state: \(controller.state.description)")
            Text("Output: \(controller.currentOutput)")
                .onAppear {
                    controller.startMonitoringAudioRoute()
                }
                .onDisappear {
                    controller.stopMonitoringAudioRoute()
                }
            Text("Gstreamer message: \(controller.backendMessage)")
            Spacer()

            HStack {
                ForEach(playlists) { playlist in
                    Button(action: {
                        selectPlaylist(playlist: playlist)
                    }) {
                        Text(playlist.name)
                            .foregroundColor(.white)
                    }
                    .padding(.all, 8)
                    .background {
                        RoundedRectangle(cornerRadius: 24.0)
                            .foregroundColor(.indigo)
                    }
                }
            }
            Spacer()
            Button("Play/pause") {
                togglePlay()
            }
            .buttonStyle(.borderedProminent)
        }
    }

    func selectPlaylist(playlist: Playlist) {
        let request = PlayRequest(uri: playlist.uri)
        NetworkManager.sendRequest(with: request, to: "http://10.0.0.153:7755/play", method: .POST).sink(receiveCompletion: { completion in
            switch completion {
            case .failure(let error):
                print("Error: \(error.localizedDescription)")
            case .finished:
                break
            }
        }, receiveValue: { data in
            print("Response: \(String(data: data, encoding: .utf8) ?? "Invalid response")")
        })
//        .store(in: &cancellables)
    }

    func togglePlay() {
        switch controller.state {
        case .playing:
            controller.pause()
        case .paused:
            controller.play()
        case .initializing:
            break
        case .ready:
            controller.play()
        }
    }
}

struct PlayRequest: Encodable {
    let uri: String
}

struct Playlist: Identifiable {
    let id = UUID()
    let name: String
    let uri: String

    init(_ name: String, _ uri: String) {
        self.name = name
        self.uri = uri
    }
}

enum HTTPMethod: String {
    case GET
    case POST
}

enum NetworkManager {
    static func sendRequest<T: Encodable>(
        with object: T?,
        to url: String,
        method: HTTPMethod
    ) -> AnyPublisher<Data, URLError> {
        guard let url = URL(string: url) else {
            return Fail(error: URLError(.requestBodyStreamExhausted)).eraseToAnyPublisher()
        }

        var request = URLRequest(url: url)
        request.httpMethod = method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let object = object, method == .POST {
            do {
                let jsonData = try JSONEncoder().encode(object)
                request.httpBody = jsonData
            } catch {
                return Fail(error: URLError(.requestBodyStreamExhausted)).eraseToAnyPublisher()
            }
        }

        return URLSession.shared.dataTaskPublisher(for: request)
            .map { $0.data }
            .receive(on: RunLoop.main)
            .eraseToAnyPublisher()
    }
}
