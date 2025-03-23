import Combine
import Foundation
import SwiftUI

enum PlayerState: String, Decodable {
    case playing = "Playing"
    case stopped = "Stopped"
    case paused = "Paused"
}

struct PlayerStatus: Decodable {
    let sleep_timer: Int?
    let status: PlayerState
    let shuffle: Bool
    let metadata: MusicMetadata?
    
    var playing: Bool {
        self.status == .playing
    }
}

struct MusicMetadata: Decodable {
    let artist: String
    let title: String
    let artwork_url: String
}

class MusicController: NSObject, ObservableObject {
    @Published var updateStatus = true
    @Published var status: PlayerStatus = .init(sleep_timer: nil, status: .stopped, shuffle: false, metadata: nil)

    private var cancellables = Set<AnyCancellable>()
    private var statusStreamTask: URLSessionDataTask?
    
    private var sseSession: URLSession?
    private var eventBuffer = ""
    
    private let sseDelegateQueue: OperationQueue = {
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        return queue
    }()

    override init() {
        super.init()
        NotificationCenter.default.addObserver(self, selector: #selector(appDidBecomeActive),
                                               name: UIApplication.didBecomeActiveNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(appDidEnterBackground),
                                               name: UIApplication.didEnterBackgroundNotification, object: nil)
        startStatusStream()
    }

    deinit {
         NotificationCenter.default.removeObserver(self)
         stopStatusStream()
     }

     @objc private func appDidBecomeActive() {
         startStatusStream()
     }

     @objc private func appDidEnterBackground() {
         stopStatusStream()
     }

     private func startStatusStream() {
         guard let url = URL(string: "http://\(SleepStreamApp.SERVER):7755/status_stream") else { return }
         var request = URLRequest(url: url)
         request.addValue("text/event-stream", forHTTPHeaderField: "Accept")
         sseSession = URLSession(configuration: .default, delegate: self, delegateQueue: sseDelegateQueue)
         statusStreamTask = sseSession?.dataTask(with: request)
         statusStreamTask?.resume()
     }

     private func stopStatusStream() {
         statusStreamTask?.cancel()
         statusStreamTask = nil
         sseSession?.invalidateAndCancel()
         sseSession = nil
     }

    func play() {
        controlPlayer("play")
    }

    func pause() {
        controlPlayer("pause")
    }

    func next() {
        controlPlayer("next")
    }

    func previous() {
        controlPlayer("previous")
    }

    func setShuffle(_ shuffle: Bool) {
        let url = "http://\(SleepStreamApp.SERVER):7755/shuffle"
        let request = ShuffleRequestDto(shuffle: shuffle)

        NetworkService.sendRequest(with: request, to: url, method: .POST)
            .decode(type: PlayerStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { completion in
                if case .failure(let error) = completion {
                    print("Error: \(error.localizedDescription)")
                }
            }, receiveValue: { [weak self] data in
                self?.updateStatusData(data)
            })
            .store(in: &cancellables)
    }

    func startSleepTimer(_ seconds: Int) {
        let url = "http://\(SleepStreamApp.SERVER):7755/sleep"
        let request = SleepTimerRequestDto(timer: seconds)

        NetworkService.sendRequest(with: request, to: url, method: .POST)
            .decode(type: PlayerStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { completion in
                if case .failure(let error) = completion {
                    print("Error: \(error.localizedDescription)")
                }
            }, receiveValue: { [weak self] data in
                self?.updateStatusData(data)
            })
            .store(in: &cancellables)
    }

    private func controlPlayer(_ action: String) {
        let url = "http://\(SleepStreamApp.SERVER):7755/\(action)"
        let request = ActionRequestDto(action: action)

        NetworkService.sendRequest(with: request, to: url, method: .POST)
            .decode(type: PlayerStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { completion in
                if case .failure(let error) = completion {
                    print("Error: \(error.localizedDescription)")
                }
            }, receiveValue: { [weak self] data in
                self?.updateStatusData(data)
            })
            .store(in: &cancellables)
    }

    func updateStatusData(_ status: PlayerStatus) {
        DispatchQueue.main.async {
            self.status = status
        }
    }
}

/// URLSessionDataDelegate for SSE Handling
extension MusicController: URLSessionDataDelegate {
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard let newData = String(data: data, encoding: .utf8) else { return }
        eventBuffer.append(newData)
        
        let events = eventBuffer.components(separatedBy: "\n\n")
        eventBuffer = events.last ?? ""
        
        for event in events.dropLast() {
            if event.hasPrefix("data:") {
                let jsonString = event.replacingOccurrences(of: "data:", with: "").trimmingCharacters(in: .whitespacesAndNewlines)
                if let jsonData = jsonString.data(using: .utf8) {
                    do {
                        let playerStatus = try JSONDecoder().decode(PlayerStatus.self, from: jsonData)
                        updateStatusData(playerStatus)
                    } catch {
                        print("Error decoding SSE event: \(error)")
                    }
                }
            }
        }
    }
}
