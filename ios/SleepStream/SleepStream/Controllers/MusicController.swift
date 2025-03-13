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

class MusicController: ObservableObject {
    @Published var updateStatus = true
    @Published var status: PlayerStatus = .init(sleep_timer: nil, status: .stopped, shuffle: false, metadata: nil)

    private var cancellables = Set<AnyCancellable>()
    private var statusTimer: DispatchSourceTimer?

    init() {
        NotificationCenter.default.addObserver(self, selector: #selector(appDidBecomeActive), name: UIApplication.didBecomeActiveNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(appDidEnterBackground), name: UIApplication.didEnterBackgroundNotification, object: nil)
        startStatusUpdate()
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
        stopStatusUpdate()
    }

    @objc private func appDidBecomeActive() {
        startStatusUpdate()
    }

    @objc private func appDidEnterBackground() {
        stopStatusUpdate()
    }

    private func startStatusUpdate() {
        let queue = DispatchQueue.global(qos: .background)
        statusTimer = DispatchSource.makeTimerSource(queue: queue)
        statusTimer?.schedule(deadline: .now(), repeating: 1.0)

        statusTimer?.setEventHandler { [weak self] in
            self?.statusUpdate()
        }

        statusTimer?.resume()
    }

    private func stopStatusUpdate() {
        statusTimer?.cancel()
        statusTimer = nil
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
                switch completion {
                case .failure(let error):
                    print("Error: \(error.localizedDescription)")
                case .finished:
                    break
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
                switch completion {
                case .failure(let error):
                    print("Error: \(error.localizedDescription)")
                case .finished:
                    break
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
                switch completion {
                case .failure(let error):
                    print("Error: \(error.localizedDescription)")
                case .finished:
                    break
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

    func statusUpdate() {
        if !updateStatus {
            return
        }

        let url = "http://\(SleepStreamApp.SERVER):7755/status"
        NetworkService.sendRequest(with: EmptyBody?(nil), to: url, method: .GET)
            .decode(type: PlayerStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { [weak self] completion in
                switch completion {
                case .failure(let error):
                    print("Failed to fetch music status: \(error)")
                case .finished:
                    break
                }
            }, receiveValue: { [weak self] data in
                self?.updateStatusData(data)
            })
            .store(in: &cancellables)
    }
}
