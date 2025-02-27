import Combine
import Foundation
import KeychainAccess
import SpotifyWebAPI
import UIKit

/**
 A helper class that wraps around an instance of `SpotifyAPI` and provides
 convenience methods for authorizing your application.
 */
final class SpotifyController: ObservableObject {
    
    private static let clientId: String = "fc4ccd0248b948cb8a5f19d594dfba0d"
    private static let clientSecret: String = "083ff619a9814a56a7d96ce31019c188"
    
    /// The key in the keychain that is used to store the authorization
    /// information: "authorizationManager".
    static let authorizationManagerKey = "authorizationManager"
    
    /// The key in the keychain that is used to store the authorization
    /// information: "authorizationManager".
    let authorizationManagerKey = "authorizationManager"
      
    /// The URL that Spotify will redirect to after the user either authorizes
    /// or denies authorization for your application.
    let loginCallbackURL = URL(
        string: "sleepstream://spotify-login-callback"
    )!
      
    /// A cryptographically-secure random string used to ensure than an incoming
    /// redirect from Spotify was the result of a request made by this app, and
    /// not an attacker. **This value is regenerated after each authorization**
    /// **process completes.**
    var authorizationState = String.randomURLSafe(length: 128)
      
    /**
     Whether or not the application has been authorized. If `true`, then you can
     begin making requests to the Spotify web API using the `api` property of
     this class, which contains an instance of `SpotifyAPI`.

     When `false`, `LoginView` is presented, which prompts the user to login.
     When this is set to `true`, `LoginView` is dismissed.

     This property provides a convenient way for the user interface to be
     updated based on whether the user has logged in with their Spotify account
     yet. For example, you could use this property disable UI elements that
     require the user to be logged in.

     This property is updated by `authorizationManagerDidChange()`, which is
     called every time the authorization information changes, and
     `authorizationManagerDidDeauthorize()`, which is called every time
     `SpotifyAPI.authorizationManager.deauthorize()` is called.
     */
    @Published var isAuthorized = false
      
    /// If `true`, then the app is retrieving access and refresh tokens. Used by
    /// `LoginView` to present an activity indicator.
    @Published var isRetrievingTokens = false
      
    @Published var currentUser: SpotifyUser? = nil
    @Published var playlists: [Playlist] = []
      
    /// The keychain to store the authorization information in.
    let keychain = Keychain(service: "com.bitechular.SleepStream")
      
    /// An instance of `SpotifyAPI` that you use to make requests to the Spotify
    /// web API.
    let api = SpotifyAPI(
        authorizationManager: AuthorizationCodeFlowManager(
            clientId: SpotifyController.clientId,
            clientSecret: SpotifyController.clientSecret
        )
    )
      
    var cancellables: Set<AnyCancellable> = []

    // MARK: - Methods -
      
    init() {
        // Configure the loggers.
        self.api.apiRequestLogger.logLevel = .trace
        // self.api.logger.logLevel = .trace
          
        // MARK: Important: Subscribe to `authorizationManagerDidChange` BEFORE

        // MARK: retrieving `authorizationManager` from persistent storage

        self.api.authorizationManagerDidChange
            // We must receive on the main thread because we are updating the
            // @Published `isAuthorized` property.
            .receive(on: RunLoop.main)
            .sink(receiveValue: self.authorizationManagerDidChange)
            .store(in: &self.cancellables)
          
        self.api.authorizationManagerDidDeauthorize
            .receive(on: RunLoop.main)
            .sink(receiveValue: self.authorizationManagerDidDeauthorize)
            .store(in: &self.cancellables)
          
        // MARK: Check to see if the authorization information is saved in

        // MARK: the keychain.

        if let authManagerData = keychain[data: self.authorizationManagerKey] {
              
            do {
                // Try to decode the data.
                let authorizationManager = try JSONDecoder().decode(
                    AuthorizationCodeFlowManager.self,
                    from: authManagerData
                )
                print("found authorization information in keychain")
                self.api.authorizationManager = authorizationManager
                  
            } catch {
                print("Could not decode authorizationManager from data:\n\(error)")
            }
        } else {
            print("Did NOT find authorization information in keychain")
        }
    }
      
    /**
     A convenience method that creates the authorization URL and opens it in the
     browser.

     You could also configure it to accept parameters for the authorization
     scopes.

     This is called when the user taps the "Log in with Spotify" button in
     `LoginView`.
     */
    func authorize() {
        let url = self.api.authorizationManager.makeAuthorizationURL(
            redirectURI: self.loginCallbackURL,
            showDialog: true,
            state: self.authorizationState,
            scopes: [
                .userReadPlaybackState,
                .userModifyPlaybackState,
                .playlistReadPrivate,
                .playlistReadCollaborative,
                .userLibraryRead,
                .userLibraryModify,
                .userReadRecentlyPlayed
            ]
        )!
          
        UIApplication.shared.open(url)
    }
      
    /**
     Saves changes to `api.authorizationManager` to the keychain.

     This method is called every time the authorization information changes. For
     example, when the access token gets automatically refreshed, (it expires
     after an hour) this method will be called.

     It will also be called after the access and refresh tokens are retrieved
     using `requestAccessAndRefreshTokens(redirectURIWithQuery:state:)`.

     Read the full documentation for
     [SpotifyAPI.authorizationManagerDidChange][1].

     [1]: https://peter-schorn.github.io/SpotifyAPI/Classes/SpotifyAPI.html#/s:13SpotifyWebAPI0aC0C29authorizationManagerDidChange7Combine18PassthroughSubjectCyyts5NeverOGvp
     */
    func authorizationManagerDidChange() {
          
        self.isAuthorized = self.api.authorizationManager.isAuthorized()
        if self.isAuthorized {
            self.updatePlaylists()
        }
        
        self.retrieveCurrentUser()
          
        do {
            // Encode the authorization information to data.
            let authManagerData = try JSONEncoder().encode(
                self.api.authorizationManager
            )
              
            // Save the data to the keychain.
            self.keychain[data: self.authorizationManagerKey] = authManagerData
              
        } catch {
            print("Couldn't encode authorizationManager for storage in keychain:\n\(error)")
        }
    }
      
    func updatePlaylists() {
        self.api.currentUserPlaylists()
            .extendPagesConcurrently(self.api)
            .collectAndSortByOffset()
            .sink(
                receiveCompletion: { _ in },
                receiveValue: { lists in
                    var result: [Playlist] = []
                    for list in lists {
                        if list.name.lowercased().contains("sleep") {
                            result.append(Playlist(list.name, list.uri, list.images[0].url))
                        }
                    }
                    DispatchQueue.main.async {
                        self.playlists = result
                    }
                }
            )
            .store(in: &self.cancellables)
    }
    
    /**
     Removes `api.authorizationManager` from the keychain and sets `currentUser`
     to `nil`.

     This method is called every time `api.authorizationManager.deauthorize` is
     called.
     */
    func authorizationManagerDidDeauthorize() {
          
        self.isAuthorized = false
          
        self.currentUser = nil
          
        do {
            /*
             Remove the authorization information from the keychain.

             If you don't do this, then the authorization information that you
             just removed from memory by calling
             `SpotifyAPI.authorizationManager.deauthorize()` will be retrieved
             again from persistent storage after this app is quit and
             relaunched.
             */
            try self.keychain.remove(self.authorizationManagerKey)
            print("did remove authorization manager from keychain")
              
        } catch {
            print(
                "couldn't remove authorization manager " +
                    "from keychain: \(error)"
            )
        }
    }

    /**
     Retrieve the current user.
       
     - Parameter onlyIfNil: Only retrieve the user if `self.currentUser`
           is `nil`.
     */
    func retrieveCurrentUser(onlyIfNil: Bool = true) {
          
        if onlyIfNil, self.currentUser != nil {
            return
        }

        guard self.isAuthorized else { return }

        self.api.currentUserProfile()
            .receive(on: RunLoop.main)
            .sink(
                receiveCompletion: { completion in
                    if case .failure(let error) = completion {
                        print("couldn't retrieve current user: \(error)")
                    }
                },
                receiveValue: { user in
                    self.currentUser = user
                }
            )
            .store(in: &self.cancellables)
    }
}
