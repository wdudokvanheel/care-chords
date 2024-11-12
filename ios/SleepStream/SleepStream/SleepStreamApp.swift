import Combine
import SwiftUI

@main
struct SleepStreamApp: App {
    var audioViewModel: AudioPlayerViewModel
    @State private var cancellables: Set<AnyCancellable> = []

    init() {
        setenv("GST_DEBUG", "4", 1)
        setenv("GST_DEBUG_NO_COLOR", "1", 1)

        print("Init gst")
        gst_ios_init()
        print("Init gst done")

        let spotify: SpotifyController = .init()
        audioViewModel = .init(spotify: spotify)
    }

    var body: some Scene {
        WindowGroup {
            AudioPlayerView()
                .environmentObject(audioViewModel)
                .onOpenURL(perform: handleURL(_:))
        }
    }

    func handleURL(_ url: URL) {
        let spotify = audioViewModel.spotify
        guard url.scheme == spotify.loginCallbackURL.scheme else {
            print("not handling URL: unexpected scheme: '\(url)'")
            return
        }

        // This property is used to display an activity indicator in `LoginView`
        // indicating that the access and refresh tokens are being retrieved.
        spotify.isRetrievingTokens = true

        // Complete the authorization process by requesting the access and
        // refresh tokens.
        spotify.api.authorizationManager.requestAccessAndRefreshTokens(
            redirectURIWithQuery: url,
            // This value must be the same as the one used to create the
            // authorization URL. Otherwise, an error will be thrown.
            state: spotify.authorizationState
        )
        .receive(on: RunLoop.main)
        .sink(receiveCompletion: { completion in
            // Whether the request succeeded or not, we need to remove the
            // activity indicator.
            self.audioViewModel.spotify.isRetrievingTokens = false

            if case .failure(let error) = completion {
                print("couldn't retrieve access and refresh tokens:\n\(error)")
            }
        })
        .store(in: &cancellables)

        spotify.authorizationState = String.randomURLSafe(length: 128)
    }
}
