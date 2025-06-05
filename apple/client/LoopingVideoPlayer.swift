import AVKit
import SwiftUI

struct LoopingVideoPlayer: View {
    @State private var player: AVQueuePlayer
    @State private var playerLooper: AVPlayerLooper
    private var width: CGFloat
    private var height: CGFloat

    init(url: URL, width: CGFloat, height: CGFloat) {
        let asset = if #available(macOS 15, *) {
            AVURLAsset(url: url)
        } else {
            AVAsset(url: url)
        }
        let item = AVPlayerItem(asset: asset)

        let player = AVQueuePlayer(playerItem: item)
        self.player = player
        self.playerLooper = AVPlayerLooper(player: player, templateItem: item)
        self.width = width
        self.height = height

        self.player.isMuted = true
    }

    var body: some View {
        VideoPlayer(player: self.player)
            .frame(minWidth: self.width, maxWidth: .infinity, minHeight: self.height, maxHeight: .infinity, alignment: .center)
            .aspectRatio(self.width / self.height, contentMode: .fit)
            .onAppear { self.player.play() }
            .onDisappear { self.player.pause() }
            .disabled(true)
            .cornerRadius(8)
            .padding(.all, 20)
            .shadow(radius: 5)
    }
}
