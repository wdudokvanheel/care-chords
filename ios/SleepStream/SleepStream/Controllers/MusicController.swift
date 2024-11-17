import Combine
import Foundation
import SwiftUI

struct MusicStatus: Decodable {
    let playing: Bool
    let metadata: MusicMetadata?
    let sleep_timer: Int?
}

struct MusicMetadata: Decodable {
    let artist: String
    let title: String
    let artwork_url: String
}

class MusicController: ObservableObject {
    @Published var updateStatus = false
    @Published var status: MusicStatus = .init(playing: false, metadata: nil, sleep_timer: nil)

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
    
    func startSleepTimer(_ seconds: Int){
        let url = "http://10.0.0.153:7755/sleep"
        let request = SleepTimerRequestDto(timer: seconds)
        
        NetworkService.sendRequest(with: request, to: url, method: .POST)
            .decode(type: MusicStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { completion in
                switch completion {
                case .failure(let error):
                    print("Error: \(error.localizedDescription)")
                case .finished:
                    break
                }
            }, receiveValue: { [weak self] data in
                DispatchQueue.main.async {
                    self?.status = data
                }
            })
            .store(in: &cancellables)
    }

    private func controlPlayer(_ action: String) {
        let url = "http://10.0.0.153:7755/control"
        let request = ActionRequestDto(action: action)
        
        NetworkService.sendRequest(with: request, to: url, method: .POST)
            .decode(type: MusicStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { completion in
                switch completion {
                case .failure(let error):
                    print("Error: \(error.localizedDescription)")
                case .finished:
                    break
                }
            }, receiveValue: { [weak self] data in
                DispatchQueue.main.async {
                    self?.status = data
                }
            })
            .store(in: &cancellables)
    }

    func statusUpdate() {
        if !updateStatus {
            return
        }
        
        let url = "http://10.0.0.153:7755/status"
        NetworkService.sendRequest(with: EmptyBody?(nil), to: url, method: .GET)
            .decode(type: MusicStatus.self, decoder: JSONDecoder())
            .sink(receiveCompletion: { [weak self] completion in
                switch completion {
                case .failure(let error):
                    print("Failed to fetch music status: \(error)")
                case .finished:
                    break
                }
            }, receiveValue: { [weak self] response in
                DispatchQueue.main.async {
                    self?.status = response
                }
            })
            .store(in: &cancellables)
    }
}
