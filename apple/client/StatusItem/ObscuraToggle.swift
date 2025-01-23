import Cocoa
import OSLog
import SwiftUI
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "ObscuraToggle")

enum ToggleLabels: String {
    case connected = "Connected"
    case connecting = "Connecting..."
    case reconnecting = "Reconnecting..."
    case disconnecting = "Disconnecting..."
    case notConnected = "Not Connected"
}

struct ObscuraToggle: View {
    @Environment(\.openURL) private var openURL
    @ObservedObject var startupModel = StartupModel.shared
    @State private var toggleLabel = ToggleLabels.notConnected
    @State private var isToggled = false
    @State private var allowToggleSync = true
    @State private var vpnStatusId: UUID = .init()
    @State private var disconnecting = false

    let vpnStatusTimer = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    func getVpnStatus() -> NeStatus? {
        return self.startupModel.appState?.status
    }

    func toggleClick() {
        self.allowToggleSync = false
        switch self.getVpnStatus()?.vpnStatus {
        case .connected, .reconnecting:
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
                    try await self.startupModel.appState?.enableTunnel(TunnelArgs())
                    self.isToggled = true
                    self.toggleLabel = ToggleLabels.connected
                } catch {
                    logger.error("Failed to connect from status menu \(error, privacy: .public)")
                    self.toggleLabel = ToggleLabels.notConnected
                    let content = UNMutableNotificationContent()
                    if error.localizedDescription == "accountExpired" {
                        self.openURL(URLs.AppAccountPage)
                        content.body = "Your account has expired."
                    } else {
                        content.body = "An error occurred while connecting to the tunnel."
                    }
                    content.title = "Tunnel failed to connect"
                    content.interruptionLevel = .active
                    content.sound = UNNotificationSound.defaultCritical
                    displayNotification(
                        UNNotificationRequest(
                            identifier: "obscura-connect-failed",
                            content: content,
                            trigger: nil
                        )
                    )
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
                // we can't rely on @ObservedObject / @Published because it just doesn't update during connecting
                // nor we can't use onChange(...) to detet changes
                Text(self.toggleLabel.rawValue)
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
                guard let vpnStatus = self.getVpnStatus() else { return }
                if vpnStatus.version == self.vpnStatusId { return }
                self.vpnStatusId = vpnStatus.version
                switch vpnStatus.vpnStatus {
                case .connected:
                    self.isToggled = true
                    self.toggleLabel = ToggleLabels.connected
                case .connecting:
                    self.isToggled = false
                    self.toggleLabel = ToggleLabels.connecting
                case .reconnecting:
                    self.isToggled = false
                    self.toggleLabel = ToggleLabels.reconnecting
                default:
                    self.isToggled = false
                    self.toggleLabel = ToggleLabels.notConnected
                }
            }
        })
    }
}
