import Combine
import SwiftUI

@main
struct SleepStreamApp: App {
    var audioViewModel: ViewModel
    @State private var cancellables: Set<AnyCancellable> = []

    init() {
        setenv("GST_DEBUG", "4", 1)
        setenv("GST_DEBUG_NO_COLOR", "1", 1)
        gst_ios_init()

        let spotify: SpotifyController = .init()
        audioViewModel = .init(spotify: spotify)
    }

    var body: some Scene {
        WindowGroup {
            MainView()
                .environmentObject(audioViewModel)
                .onOpenURL(perform: handleURL(_:))
        }
    }

    // TODO: Move to SpotifyController
    func handleURL(_ url: URL) {
        let spotify = audioViewModel.spotify
        guard url.scheme == spotify.loginCallbackURL.scheme else {
            print("not handling URL: unexpected scheme: '\(url)'")
            return
        }
        spotify.isRetrievingTokens = true

        spotify.api.authorizationManager.requestAccessAndRefreshTokens(
            redirectURIWithQuery: url,
            state: spotify.authorizationState
        )
        .receive(on: RunLoop.main)
        .sink(receiveCompletion: { completion in
            self.audioViewModel.spotify.isRetrievingTokens = false

            if case .failure(let error) = completion {
                print("couldn't retrieve access and refresh tokens:\n\(error)")
            }
        })
        .store(in: &cancellables)

        spotify.authorizationState = String.randomURLSafe(length: 128)
    }
}
