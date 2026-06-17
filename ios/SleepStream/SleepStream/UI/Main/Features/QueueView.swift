import SwiftUI

struct QueueView: View {
    @ObservedObject var queue: QueueController
    @ObservedObject var library: AudioLibraryController

    var body: some View {
        VStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Queue")
                        .font(.headline)
                        .foregroundColor(.white)

                    Spacer()

                    if queue.isLoading {
                        ProgressView()
                            .tint(.orange)
                    }

                    Button(action: queue.load) {
                        Image(systemName: "arrow.clockwise")
                            .foregroundColor(.orange)
                    }
                    .buttonStyle(.plain)

                    Button(action: queue.clear) {
                        Image(systemName: "trash")
                            .foregroundColor(queue.items.isEmpty ? .white.opacity(0.3) : .orange)
                    }
                    .buttonStyle(.plain)
                    .disabled(queue.items.isEmpty)
                }
                .padding(.horizontal, 10)

                if let error = queue.errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                        .padding(.horizontal, 10)
                }

                if queue.items.isEmpty {
                    Text("Queue is empty")
                        .font(.subheadline)
                        .foregroundColor(Color.playlistItemLabel)
                        .frame(maxWidth: .infinity, minHeight: 72)
                        .background(Color.playlistItem)
                        .cornerRadius(8)
                        .padding(.horizontal, 8)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 6) {
                            ForEach(Array(queue.items.enumerated()), id: \.element.id) { index, item in
                                QueueRow(
                                    item: item,
                                    index: index,
                                    isCurrent: queue.state.currentIndex == index,
                                    canMoveUp: index > 0,
                                    canMoveDown: index + 1 < queue.items.count,
                                    play: { queue.play(index: index) },
                                    moveUp: { queue.moveUp(item) },
                                    moveDown: { queue.moveDown(item) },
                                    remove: { queue.remove(item) }
                                )
                            }
                        }
                        .padding(.horizontal, 8)
                    }
                    .frame(maxHeight: 260)
                }
            }
            .padding(.top, 8)

            Divider()
                .background(Color.white.opacity(0.18))

            VStack(alignment: .leading, spacing: 6) {
                Text("Add")
                    .font(.headline)
                    .foregroundColor(.white)
                    .padding(.horizontal, 10)

                AudioLibraryView(
                    library: library,
                    itemSelect: queue.enqueue,
                    collectionSelect: queue.enqueue,
                    actionIconName: "plus"
                )
            }
        }
        .onAppear {
            queue.load()
            library.load()
        }
    }
}

private struct QueueRow: View {
    let item: QueueItem
    let index: Int
    let isCurrent: Bool
    let canMoveUp: Bool
    let canMoveDown: Bool
    let play: () -> Void
    let moveUp: () -> Void
    let moveDown: () -> Void
    let remove: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Button(action: play) {
                Image(systemName: isCurrent ? "speaker.wave.2.fill" : "play.fill")
                    .font(.caption)
                    .foregroundColor(isCurrent ? .white : .orange)
                    .frame(width: 34, height: 34)
                    .background(isCurrent ? Color.orange : Color.white.opacity(0.08))
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)

            VStack(alignment: .leading, spacing: 3) {
                Text(item.title)
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .foregroundColor(.white)
                    .lineLimit(1)

                Text(label)
                    .font(.caption)
                    .foregroundColor(Color.playlistItemLabel.opacity(0.75))
                    .lineLimit(1)
            }

            Spacer(minLength: 6)

            VStack(spacing: 2) {
                Button(action: moveUp) {
                    Image(systemName: "chevron.up")
                        .font(.caption.weight(.bold))
                        .foregroundColor(canMoveUp ? .orange : .white.opacity(0.25))
                        .frame(width: 28, height: 24)
                }
                .buttonStyle(.plain)
                .disabled(!canMoveUp)

                Button(action: moveDown) {
                    Image(systemName: "chevron.down")
                        .font(.caption.weight(.bold))
                        .foregroundColor(canMoveDown ? .orange : .white.opacity(0.25))
                        .frame(width: 28, height: 24)
                }
                .buttonStyle(.plain)
                .disabled(!canMoveDown)
            }

            Button(action: remove) {
                Image(systemName: "xmark")
                    .font(.caption.weight(.bold))
                    .foregroundColor(.orange)
                    .frame(width: 32, height: 32)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(isCurrent ? Color.orange.opacity(0.28) : Color.playlistItem)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isCurrent ? Color.orange : Color.white.opacity(0.08), lineWidth: 1)
        )
        .cornerRadius(8)
    }

    private var label: String {
        let source = item.source.prefix(1).uppercased() + String(item.source.dropFirst())
        switch item.kind {
        case "file":
            return "\(source) song"
        case "folder":
            return "\(source) collection"
        case "playlist":
            return "\(source) playlist"
        default:
            return "\(source) \(item.kind)"
        }
    }
}
