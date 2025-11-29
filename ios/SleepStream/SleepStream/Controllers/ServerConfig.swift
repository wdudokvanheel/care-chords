import Foundation
import Combine

class ServerConfig: ObservableObject {
    static let shared = ServerConfig()
    
    private let key = "server_url"
    private let defaultURL = "10.0.0.20"
    
    @Published var serverURL: String {
        didSet {
            UserDefaults.standard.set(serverURL, forKey: key)
        }
    }
    
    private init() {
        self.serverURL = UserDefaults.standard.string(forKey: key) ?? defaultURL
    }
    
    func getURL() -> String {
        return serverURL
    }
    
    func saveURL(_ url: String) {
        self.serverURL = url
    }
    
    func validateConnection(url: String? = nil, completion: @escaping (Bool, String?) -> Void) -> AnyCancellable {
        let targetURL = url ?? serverURL
        let urlString = "http://\(targetURL):7755/monitor"
        guard let url = URL(string: urlString) else {
            completion(false, "Invalid URL")
            return AnyCancellable {}
        }
        
        return URLSession.shared.dataTaskPublisher(for: url)
            .map { $0.data }
            .receive(on: RunLoop.main)
            .sink(receiveCompletion: { result in
                switch result {
                case .failure(let error):
                    completion(false, error.localizedDescription)
                case .finished:
                    break
                }
            }, receiveValue: { _ in
                completion(true, nil)
            })
    }
}
