import AVKit
import NetworkExtension
import OSLog
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "startup")

struct StartupView: View {
    @StateObject var model = StartupModel.shared
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack {
            switch self.model.status {
            case .initial:
                AppIcon()
                Text("Starting Obscura")
            case .networkExtensionInit(_, .checking):
                VpnChecksView(subtext: "Checking Network Extension")
            case .networkExtensionInit(_, .enabling):
                VpnChecksView(subtext: "Enabling Network Extension")
            case .networkExtensionInit(_, .waitingForReboot):
                Text("Reboot Required")
            case .networkExtensionInit(let neInit, .blockingBeforePermissionPopup):
                InstallSystemExtensionView(startupModel: self.model, subtext: "Please allow Obscura VPN's network extension to be installed in system settings to extend the networking features of your mac.", neInit: neInit)
            case .networkExtensionInit(let neInit, .blockingBeforeTunnelDisconnect):
                UpdateSystemExtensionView(startupModel: self.model, subtext: "An updated version of Obscura VPN's network extension is required.", neInit: neInit)
            case .networkExtensionInit(_, .waitingForUserApproval):
                InstallSystemExtensionView(startupModel: self.model, subtext: "Please allow Obscura VPN's network extension to be installed in system settings to extend the networking features of your mac.")
            case .networkExtensionInit(_, .failed(let error)):
                InstallSystemExtensionView(startupModel: self.model, subtext: "Could not start the network extension. \(error). Please restart your Mac or contact support for help.")
            case .tunnelProviderInit(_, .checking):
                VpnChecksView(subtext: "Checking Tunnel Provider")
            case .tunnelProviderInit(let tpInit, .blockingBeforePermissionPopup):
                VpnConfigurationView(startupModel: self.model, subtext: "This configuration is required for Obscura VPN to anonymize your network traffic.", tpInit: tpInit)
            case .tunnelProviderInit(_, .waitingForUserApproval):
                VpnConfigurationView(startupModel: self.model, subtext: "For Obscura VPN to add itself as a VPN to your system, please click \"Allow\" in the request for permission.")
            case .tunnelProviderInit(let tpInit, .permissionDenied):
                VpnConfigurationView(startupModel: self.model, subtext: "Permission was denied. Click below to request permission again.", tpInit: tpInit, isError: true)
            case .tunnelProviderInit(_, .configuring):
                VpnChecksView(subtext: "Configuring Tunnel Provider")
            case .tunnelProviderInit(_, .testingCommunication):
                VpnChecksView(subtext: "Testing Tunnel Provider communication")
            case .tunnelProviderInit(_, .unexpectedError):
                VpnFailedView()
            case .askToRegisterLoginItem(let value):
                RegisterLoginItemView(value: value)
            case .ready:
                AppIcon()
                Text("Ready to launch")
            }
        }
    }
}

func AppIcon() -> some View {
    return Image(nsImage: NSImage(named: "AppIcon") ?? NSImage())
        .resizable()
        .frame(width: 64, height: 64)
}

struct VpnChecksView: View {
    var subtext: String

    var body: some View {
        VStack(spacing: 20) {
            AppIcon()
            ProgressView()
            Text(self.subtext)
                .font(.headline)
        }
    }
}

let macOS14DemoVideo = Bundle.main.url(forResource: "videos/macOS 14 System Extension Demo", withExtension: "mov")!
let macOS15DemoVideo = Bundle.main.url(forResource: "videos/macOS 15 System Extension Demo", withExtension: "mov")!

struct InstallSystemExtensionView: View {
    @ObservedObject var startupModel: StartupModel
    var subtext: String

    @Environment(\.openURL) private var openURL
    var neInit: NetworkExtensionInit? = nil

    var body: some View {
        ZStack {
            VStack {
                Spacer()
                Image("DecoPrimer")
                    .resizable()
                    .scaledToFit()
                    .frame(minWidth: 0, minHeight: 50)
            }
            VStack {
                Spacer()
                    .frame(minHeight: 20)
                HStack {
                    Spacer()
                    VStack(alignment: .leading, spacing: 10) {
                        Image("EmotePrimer")
                        Text("Allow System Extension")
                            .font(.title)
                        Text(self.subtext)
                            .font(.body)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                        if let neInit = self.neInit {
                            Button(action: neInit.continueAfterPriming) {
                                Text("Install Now")
                                    .font(.headline)
                                    .frame(width: 300)
                            }
                            .buttonStyle(NoFadeButtonStyle())
                        } else {
                            Button(action: {
                                if #available(macOS 15, *) {
                                    self.openURL(URLs.ExtensionSettings)
                                } else {
                                    self.openURL(URLs.PrivacySecurityExtensionSettings)
                                }
                            }) {
                                if #available(macOS 15, *) {
                                    Text("Open Login Items & Extensions Settings")
                                        .font(.headline)
                                        .frame(width: 300)
                                } else {
                                    Text("Open Privacy & Security Settings")
                                        .font(.headline)
                                        .frame(width: 300)
                                }
                            }
                            .buttonStyle(NoFadeButtonStyle())
                        }
                    }
                    .frame(width: 350)
                    .padding(.leading, 50)
                    Spacer()
                    if #available(macOS 15, *) {
                        LoopingVideoPlayer(url: macOS15DemoVideo, width: 360, height: 410)
                    } else {
                        LoopingVideoPlayer(url: macOS14DemoVideo, width: 360, height: 410)
                    }
                    Spacer()
                }
                Spacer()
                    .frame(minHeight: 50)
            }
            VStack(alignment: .trailing) {
                Spacer()
                HStack(alignment: .bottom) {
                    Spacer()
                    if #available(macOS 14.0, *) {
                        HelpLink(destination: URLs.SystemExtensionHelp)
                            .padding(.bottom, 2)
                    } else {
                        Button {
                            self.openURL(URLs.SystemExtensionHelp)
                        } label: {
                            Image(systemName: "questionmark.circle.fill")
                                .font(.system(size: 19))
                                .foregroundStyle(.white, .gray.opacity(0.4))
                        }
                        .buttonStyle(.plain)
                        .padding(.bottom, 2)
                        .padding(.trailing, 2)
                    }
                }
                .padding()
            }
        }
    }
}

struct UpdateSystemExtensionView: View {
    @ObservedObject var startupModel: StartupModel
    var subtext: String

    @Environment(\.openURL) private var openURL
    var neInit: NetworkExtensionInit

    var body: some View {
        Spacer()
            .frame(height: 60)
        // extensions symbol for macOS <= 15
        // coincidentally used for the network extensions symbol on macOS 15
        Image(systemName: "puzzlepiece.extension.fill")
            .font(.system(size: 48))
            .padding()
        Text("System Extension Update Required")
            .font(.title)
        Text(self.subtext)
            .font(.body)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(width: 350)
            .padding()

        Button(action: self.neInit.continueAfterPriming) {
            Text("Disconnect and Update")
                .font(.headline)
                .frame(width: 300)
        }
        .buttonStyle(NoFadeButtonStyle())
    }
}

struct VpnConfigurationView: View {
    @ObservedObject var startupModel: StartupModel
    var subtext = ""

    @Environment(\.openURL) private var openURL
    var tpInit: TunnelProviderInit? = nil
    var isError = false

    var body: some View {
        let primer = self.tpInit != nil && !self.isError
        ZStack(alignment: .topLeading) {
            Image(systemName: "network.badge.shield.half.filled")
                .font(.system(size: 48))
                .foregroundStyle(.blue)
                .buttonStyle(.plain)
                .opacity(primer ? 1 : 0)
            Image(systemName: "network")
                .font(.system(size: 48))
                .foregroundStyle(.blue)
                .buttonStyle(.plain)
                .opacity(primer ? 0 : 1)
                .overlay(alignment: .bottomTrailing) {
                    Image(systemName: self.isError ? "xmark.circle.fill" : "ellipsis.circle.fill")
                        .font(.system(size: 19))
                        .foregroundStyle(.black, self.isError ? .red : .white)
                        .opacity(primer ? 0 : 1)
                        .alignmentGuide(.bottom, computeValue: { $0.height })
                        .alignmentGuide(.trailing, computeValue: { $0.width })
                }
        }
        .padding()

        Text("Allow VPN Configuration")
            .font(.title)
        if !self.subtext.isEmpty {
            Text(self.subtext)
                .padding()
                .italic()
                .frame(width: 350)
                .frame(minHeight: 100)
                .multilineTextAlignment(.center)
        }
        Button(action: { self.tpInit?.continueAfterPermissionPriming() }) {
            Text(self.isError ? "Retry VPN Configuration" : "Allow VPN Configuration")
                .font(.headline)
                .frame(width: 300)
        }
        .buttonStyle(NoFadeButtonStyle())
        .disabled(!primer && !self.isError)
    }
}

struct VpnFailedView: View {
    var body: some View {
        VStack(spacing: 20) {
            AppIcon()
            Image(systemName: "xmark.circle.fill")
                .font(.system(size: 40))
                .foregroundStyle(.black, .red)
                .padding()
            Text("Problem initializing Tunnel Provider")
                .font(.headline)
            Text("Please try restarting your Mac or contact support for help.")
        }
    }
}

struct RegisterLoginItemView: View {
    var value: ObservableValue<Bool>
    @Environment(\.openURL) private var openURL
    @State private var isRegistering = false

    var body: some View {
        Image(systemName: "desktopcomputer.and.arrow.down")
            .font(.system(size: 48))
            .symbolRenderingMode(.palette)
            .foregroundStyle(.white, .blue)
            .padding()
            .buttonStyle(.plain)

        Text("Open at Login")
            .font(.title)

        Text("Do you want Obscura VPN to open automatically when you log in?")
            .font(.body)
            .multilineTextAlignment(.center)
            .padding()

        if self.isRegistering {
            ProgressView()
        } else {
            Button(action: { self.value.publish(true) }) {
                Text("Yes")
                    .font(.headline)
                    .frame(width: 300)
            }
            .buttonStyle(NoFadeButtonStyle())

            Button(action: { self.value.publish(false) }) {
                Text("No")
                    .frame(width: 300)
            }
            .buttonStyle(NoFadeButtonStyle(backgroundColor: Color(.darkGray)))
        }
    }
}

enum StartupStatus {
    case initial
    case networkExtensionInit(NetworkExtensionInit, NetworkExtensionInitStatus)
    case tunnelProviderInit(TunnelProviderInit, TunnelProviderInitStatus)
    case askToRegisterLoginItem(ObservableValue<Bool>)
    case ready
}

class StartupModel: ObservableObject {
    static let shared = StartupModel()

    @Published var status = StartupStatus.initial
    @Published var appState: AppState?

    init() {
        self.start()
    }

    private func start() {
        Task { @MainActor in
            guard let () = await self.stepNetworkExtensionInit() else {
                return
            }

            guard let (tunnelProviderManager, status) = await self.stepTunnelProviderInit() else {
                return
            }

            await self.stepRegisterLoginItem()

            self.update(status: .ready)
            self.appState = AppState(tunnelProviderManager, initialStatus: status)
        }
    }

    @MainActor private func update(status: StartupStatus) {
        logger.info("StartupModel.status = \(debugFormat(status), privacy: .public)")
        self.status = status
    }

    @MainActor private func stepNetworkExtensionInit() async -> Void? {
        var tunnelConnected = false
        do {
            let managers: [NETunnelProviderManager] = try await NETunnelProviderManager.loadAllFromPreferences()
            for manager in managers {
                let status = manager.connection.status
                if status != .disconnected {
                    logger.info("connection status is \(status, privacy: .public), assume tunnel is connected")
                    tunnelConnected = true
                }
            }
        } catch {
            logger.error("could not determine connection status, assume tunnel is connected: \(error, privacy: .public)")
            tunnelConnected = true
        }

        let neInit = NetworkExtensionInit(tunnelConnected: tunnelConnected)
        for await event in neInit.start() {
            switch event {
            case .status(let status):
                self.update(status: .networkExtensionInit(neInit, status))
            case .done:
                return ()
            }
        }
        logger.error("Failed to initialize network extension! \(debugFormat(self.status), privacy: .public)")
        return nil
    }

    @MainActor private func stepTunnelProviderInit() async -> (NETunnelProviderManager, NeStatus)? {
        let tpInit = TunnelProviderInit()
        for await event in tpInit.start() {
            switch event {
            case .status(let status):
                self.update(status: .tunnelProviderInit(tpInit, status))
            case .done(let manager, let status):
                return (manager, status)
            }
        }
        logger.error("Failed to initialize tunnel provider! \(debugFormat(self.status), privacy: .public)")
        return nil
    }

    @MainActor private func stepRegisterLoginItem() async {
        if !UserDefaults.standard.bool(forKey: UserDefaultKeys.LoginItemRegistered) {
            let value = ObservableValue<Bool>()
            self.update(status: .askToRegisterLoginItem(value))
            if await value.get() {
                do {
                    try registerAsLoginItem()
                } catch {}
            }
            UserDefaults.standard.set(true, forKey: UserDefaultKeys.LoginItemRegistered)
        }
    }
}

struct NoFadeButtonStyle: ButtonStyle {
    var backgroundColor: Color = .init("ObscuraOrange")
    @Environment(\.isEnabled) private var isEnabled: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding()
            .background(self.isEnabled ? self.backgroundColor : Color.gray)
            .foregroundColor(.white)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.snappy(duration: 0.2), value: configuration.isPressed)
    }
}
