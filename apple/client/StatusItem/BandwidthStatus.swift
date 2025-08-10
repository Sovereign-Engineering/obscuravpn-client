import Cocoa
import SwiftUI

class BandwidthStatusModel: ObservableObject {
    @Published var uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
    @Published var downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
    @Published var exitRTT: Duration?
}

struct BandwidthStatusItem: View {
    var isUpload: Bool
    var bandwidth: BandwidthFmt
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        HStack {
            Image(systemName: self.isUpload ? "arrow.up" : "arrow.down")
            Text(self.isUpload ? "Upload" : "Download")
            Spacer()
            HStack {
                Text("\(self.bandwidth.TransferPerSecond) \(self.bandwidth.MeasurementUnit)")
                    .monospaced()
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 8)
        .background(self.colorScheme == .dark ? Color.white.opacity(0.1) : Color.black.opacity(0.05))
        .cornerRadius(5)
        .shadow(radius: 2)
    }
}

struct ExitRTTStatusItem: View {
    var exitRTT: Duration?
    @Environment(\.colorScheme) var colorScheme

    var exitRTTText: String {
        guard let exitRTT = exitRTT else {
            return "unknown"
        }
        return exitRTT.formatted(.units(allowed: [.milliseconds], width: .condensedAbbreviated))
    }

    var body: some View {
        HStack {
            Image(systemName: "timer")
            Text("Exit RTT")
            Spacer()
            HStack {
                Text(self.exitRTTText)
                    .monospaced()
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 8)
        .background(self.colorScheme == .dark ? Color.white.opacity(0.1) : Color.black.opacity(0.05))
        .cornerRadius(5)
        .shadow(radius: 2)
    }
}

struct BandwidthStatus: View {
    @ObservedObject var bandwidthStatusModel: BandwidthStatusModel
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        VStack {
            BandwidthStatusItem(isUpload: true, bandwidth: self.bandwidthStatusModel.uploadBandwidth)
            BandwidthStatusItem(isUpload: false, bandwidth: self.bandwidthStatusModel.downloadBandwidth)
            ExitRTTStatusItem(exitRTT: self.bandwidthStatusModel.exitRTT)
        }
        .padding(EdgeInsets(top: 5, leading: 12, bottom: 5, trailing: 12))
    }
}

struct BandwidthFmt {
    let TransferPerSecond: String
    // TB/s, GB/s, MB/s, KB/s
    let MeasurementUnit: String
    let Intensity: Int

    static func fromTransferRate(bytesPerSecond: Double) -> BandwidthFmt {
        var divisor: Double = 1
        var unit = " B/s"
        var intensityLvl = 0

        if bytesPerSecond >= 1_000_000_000_000 {
            divisor = 1_000_000_000_000
            unit = "TB/s"
            intensityLvl = BANDWIDTH_MAX_INTENSITY
        } else if bytesPerSecond >= 1_000_000_000 {
            divisor = 1_000_000_000
            unit = "GB/s"
            intensityLvl = BANDWIDTH_MAX_INTENSITY
        } else if bytesPerSecond >= 1_000_000 {
            divisor = 1_000_000
            unit = "MB/s"
            if bytesPerSecond >= 200_000_000 {
                // 200+ MB/s is basically max bars, arbitrary but loosely backed
                // e.g. Steam tops out near 250 MB/s
                // https://www.reddit.com/r/Steam/comments/10nhtsr/testing_the_limits_of_what_download_speeds_steam/
                intensityLvl = BANDWIDTH_MAX_INTENSITY
            } else if bytesPerSecond > 100_000_000 {
                intensityLvl = BANDWIDTH_MAX_INTENSITY - 1
            } else if bytesPerSecond > 20_000_000 {
                intensityLvl = BANDWIDTH_MAX_INTENSITY - 2
            } else {
                intensityLvl = 1
            }
        } else if bytesPerSecond >= 100 {
            divisor = 1000
            unit = "KB/s"
            intensityLvl = bytesPerSecond >= 10000 ? 1 : 0
        }

        let transferRate = bytesPerSecond / divisor
        var transferPerSecond: String
        if transferRate >= 100 {
            transferPerSecond = String(Int(transferRate))
        } else {
            // round to one decimal place (e.g. 10.1 KB/s, 1.1 KB/s, 0.1 KB/s)
            transferPerSecond = String((transferRate * 10).rounded() / 10)
        }
        return BandwidthFmt(TransferPerSecond: leftPad(transferPerSecond, toLength: 4, withPad: "\u{2007}"), MeasurementUnit: unit, Intensity: intensityLvl)
    }
}
