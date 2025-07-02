import Cocoa
import OSLog
import SwiftUI
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "ObscuraToggle")

enum ToggleLabels: String {
    case connected
    case connecting
    case reconnecting
    case disconnecting
    case notConnected
}

struct ObscuraToggle: View {
    @Environment(\.openURL) private var openURL
    @ObservedObject var startupModel = StartupModel.shared
    @ObservedObject var osStatusModel: OsStatusModel
    @State private var toggleLabel = ToggleLabels.notConnected
    @State private var isToggled = false
    @State private var allowToggleSync = true
    @State private var vpnStatusId: UUID = .init()
    @State private var disconnecting = false

    let vpnStatusTimer = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    func getVpnStatus() -> NeStatus? {
        return self.startupModel.appState?.status
    }

    func getCityName() -> String? {
        switch self.getVpnStatus()?.vpnStatus {
        case .connected(_, let exit, _, _, _, _):
            return exit.city_name
        default:
            return nil
        }
    }

    func getToggleText() -> String {
        switch self.toggleLabel {
        case .connected:
            let cityName = self.getCityName()
            if cityName == nil {
                return "Connected"
            }
            return "Connected to \(cityName!)"
        case .connecting: return "Connecting..."
        case .reconnecting: return "Reconnecting..."
        case .disconnecting: return "Disconnecting..."
        // adding tabs prevents text overflow on the first status menu connect
        case .notConnected: return "Not Connected\t\t\t"
        }
    }

    func toggleClick() {
        self.allowToggleSync = false
        switch self.getVpnStatus()?.vpnStatus {
        case .connected:
            self.isToggled = false
            // this returns faster than the UI could show "Disconnecting"
            self.toggleLabel = ToggleLabels.disconnecting
            self.startupModel.appState?.disableTunnel()
            // since disconnect is fairly instant, we only need to delay the toggle sync for a bit
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                // if for some reason the vpn is connected right after a disconnect,
                // and we don't disable the override flag, we wil
                self.allowToggleSync = true
            }
        default:
            Task {
                self.toggleLabel = ToggleLabels.connecting
                do {
                    let exitSelector = self.getVpnStatus()?.lastChosenExit ?? .any
                    try await self.startupModel.appState?.enableTunnel(TunnelArgs(exit: exitSelector))
                } catch {
                    logger.error("Failed to connect from status menu toggle \(error, privacy: .public)")
                    self.toggleLabel = ToggleLabels.notConnected
                }
                self.allowToggleSync = true
            }
        }
    }

    // we're implicitly creating a (calculated) minimum width here with
    //   - .fixedSize(...)
    //   - Spacer(minLength: 54)
    var body: some View {
        // Separate the presentation from the function to avoid
        // https://stackoverflow.com/a/59398852/7732434
        let toggleBind = Binding<Bool>(
            get: { self.isToggled },
            set: { _ in
                self.toggleClick()
            }
        )

        HStack {
            VStack(alignment: .leading) {
                Text("Obscura VPN")
                    .font(.headline.weight(.regular))
                Text(self.getToggleText())
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .lineLimit(1)
            // so that the text doesn't collapse horizontally and truncate
            .fixedSize(horizontal: true, vertical: false)
            Spacer(minLength: 54)
            Toggle(isOn: toggleBind) {}
                .toggleStyle(.switch)
                .tint(Color("ObscuraOrange"))
                .disabled(self.toggleLabel == ToggleLabels.disconnecting)
        }
        // this allows the Spacer to be clickable
        .contentShape(Rectangle())
        // leading and trailing matches Tailscale's values as observed via Accessibility Inspector
        .padding(EdgeInsets(top: 5, leading: 14, bottom: 5, trailing: 12))
        .onTapGesture { self.toggleClick() }
        .onReceive(self.vpnStatusTimer, perform: { _ in
            if self.allowToggleSync {
                if self.osStatusModel.osStatus?.osVpnStatus == .disconnecting {
                    self.isToggled = false
                    self.toggleLabel = ToggleLabels.disconnecting
                    return
                }
                guard let vpnStatus = self.getVpnStatus() else { return }
                // Don't update the toggle's state if the state has already been updated for a particular vpnStatus
                // This avoids bugs where the toggle is the component driving a vpn status change
                // E.g. The vpnStatus reports disconnected and the user starts a connection through the toggle
                //  -> Show the connecting state until the new vpnStatus rather than showing a disconnected state
                if vpnStatus.version == self.vpnStatusId { return }
                self.vpnStatusId = vpnStatus.version
                switch vpnStatus.vpnStatus {
                case .connected:
                    self.isToggled = true
                    self.toggleLabel = ToggleLabels.connected
                case .connecting(tunnelArgs: _, connectError: _, reconnecting: let reconnecting):
                    self.isToggled = false
                    self.toggleLabel = reconnecting ? ToggleLabels.reconnecting : ToggleLabels.connecting
                default:
                    self.isToggled = false
                    self.toggleLabel = ToggleLabels.notConnected
                }
            }
        })
    }
}
