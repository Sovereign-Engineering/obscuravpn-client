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
    @State private var isHovering = false
    @State private var cityNames: [CityExit: String] = [:]

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

    func getConnectHint() -> String {
        let exitSelector = self.getVpnStatus()?.lastExit

        switch exitSelector {
        case .city(let countryCode, let cityCode):
            let cityExit = CityExit(city_code: cityCode, country_code: countryCode)
            if let cityName = self.cityNames[cityExit] {
                return "Connect to \(cityName), \(countryCode.uppercased())"
            }
            return cityCode
        case .country(let countryCode):
            return "Connect to \(countryCode.uppercased())"
        case .exit(let exitId):
            return exitId
        default:
            return "Connect via Quick Connect"
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
        case .connecting:
            if self.isHovering {
                return "Click to Cancel"
            }
            return "Connecting..."
        case .reconnecting:
            if self.isHovering {
                return "Click to Cancel"
            }
            return "Reconnecting..."
        case .disconnecting: return "Disconnecting..."
        case .notConnected:
            if self.isHovering {
                return self.getConnectHint()
            }
            // adding tabs prevents text overflow on the first status menu connect
            return "Not Connected\t\t\t"
        }
    }

    func toggleClick() {
        self.allowToggleSync = false
        switch self.getVpnStatus()?.vpnStatus {
        case .connected, .connecting:
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
                    let exitSelector = self.getVpnStatus()?.lastExit ?? .any
                    try await self.startupModel.appState?.enableTunnel(
                        TunnelArgs(exit: exitSelector))
                } catch {
                    logger.error(
                        "Failed to connect from status menu toggle \(error, privacy: .public)")
                    self.toggleLabel = ToggleLabels.notConnected
                }
                self.allowToggleSync = true
            }
        }
    }

    var italicizeToggleLabel: Bool {
        return self.isHovering &&
            (self.toggleLabel == .notConnected
                || self.toggleLabel == .reconnecting
                || self.toggleLabel == .connecting)
    }

    // we're implicitly creating a (calculated) minimum width here with
    //   - .fixedSize(...)
    //   - Spacer(minLength: 54)
    var body: some View {
        let toggleDisabled = self.toggleLabel == ToggleLabels.disconnecting
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
                    .foregroundStyle(toggleDisabled ? .secondary : .primary)
                Text(self.getToggleText())
                    .font(.subheadline)
                    .foregroundStyle(toggleDisabled ? .tertiary : .secondary)
                    .italic(self.italicizeToggleLabel)
            }
            .lineLimit(1)
            // so that the text doesn't collapse horizontally and truncate
            .fixedSize(horizontal: true, vertical: false)
            Spacer()
            Toggle(isOn: toggleBind) {}
                .toggleStyle(.switch)
                .tint(Color("ObscuraOrange"))
                .disabled(toggleDisabled)
        }
        // this allows the Spacer to be clickable
        .contentShape(Rectangle())
        // leading and trailing matches Tailscale's values as observed via Accessibility Inspector
        .padding(EdgeInsets(top: 5, leading: 14, bottom: 5, trailing: 12))
        .onTapGesture {
            if !toggleDisabled {
                self.toggleClick()
            }
        }
        .onHover { hovering in
            self.isHovering = hovering
        }
        .onReceive(
            self.vpnStatusTimer,
            perform: { _ in
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
                    case .connecting(tunnelArgs: _, connectError: _, let reconnecting):
                        self.isToggled = false
                        self.toggleLabel =
                            reconnecting ? ToggleLabels.reconnecting : ToggleLabels.connecting
                    default:
                        self.isToggled = false
                        self.toggleLabel = ToggleLabels.notConnected
                    }
                }
            }
        )
        .task {
            var exitListKnownVersion: String?
            while true {
                var takeBreak = true
                if let appState = self.startupModel.appState {
                    do {
                        let result = try await getCityNames(
                            appState.manager, knownVersion: exitListKnownVersion
                        )
                        exitListKnownVersion = result.version
                        self.cityNames = result.cityNames
                        takeBreak = false
                    } catch {
                        logger.error(
                            "Failed to get exit list in ObscuraToggle: \(error, privacy: .public)")
                    }
                }
                if takeBreak {
                    do {
                        try await Task.sleep(seconds: 1)
                    } catch {
                        logger.error(
                            "exitListWatcher Task cancelled in ObscuraToggle \(error, privacy: .public)"
                        )
                        return
                    }
                }
            }
        }
    }
}
