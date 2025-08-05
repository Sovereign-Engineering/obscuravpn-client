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
            #if os(macOS)
                case .networkExtensionInit(_, .checking):
                    VpnChecksView(subtext: "Checking Network Extension")
                case .networkExtensionInit(_, .enabling):
                    VpnChecksView(subtext: "Enabling Network Extension")
                case .networkExtensionInit(_, .waitingForReboot):
                    Text("Reboot Required")
                case .networkExtensionInit(let neInit, .blockingBeforePermissionPopup):
                    InstallSystemExtensionView(startupModel: self.model, subtext: "Please allow Obscura VPN's network extension to be installed in System Settings to extend the networking features of your Mac.", neInit: neInit)
                case .networkExtensionInit(let neInit, .blockingBeforeTunnelDisconnect):
                    UpdateSystemExtensionView(startupModel: self.model, subtext: "An updated version of Obscura VPN's network extension is required.", neInit: neInit)
                case .networkExtensionInit(_, .waitingForUserApproval):
                    InstallSystemExtensionView(startupModel: self.model, subtext: "Please allow Obscura VPN's network extension to be installed in System Settings to extend the networking features of your Mac.")
                case .networkExtensionInit(_, .failed(let error)):
                    InstallSystemExtensionView(startupModel: self.model, subtext: "Could not start the network extension. \(error). Please restart your Mac or contact support for help.")
            #endif
            case .tunnelProviderInit(_, .checking):
                VpnChecksView(subtext: "Checking Tunnel Provider")
            case .tunnelProviderInit(let tpInit, .blockingBeforePermissionPopup):
                VpnConfigurationView(startupModel: self.model, subtext: "This configuration is required for Obscura VPN to anonymize your network traffic.", tpInit: tpInit)
            case .tunnelProviderInit(_, .waitingForUserPermissionApproval):
                VpnConfigurationView(startupModel: self.model, subtext: "For Obscura VPN to add itself as a VPN to your system, please click \"Allow\" in the request for permission. If you are currently connected to a VPN, this will disconnect it.")
            case .tunnelProviderInit(let tpInit, .permissionDenied):
                VpnConfigurationView(startupModel: self.model, subtext: "Permission was denied. Click below to request permission again.", tpInit: tpInit, isError: true)
            case .tunnelProviderInit(_, .configuring):
                VpnChecksView(subtext: "Configuring Tunnel Provider")
            case .tunnelProviderInit(let tpInit, .waitingForUserStopOtherTunnelApproval(let manager)):
                VpnEnableView(manager: manager, subtext: "Obscura VPN was disabled by another VPN. Click below to enable it. If you are currently connected to a VPN, this will disconnect it.", tpInit: tpInit)
            case .tunnelProviderInit(_, .testingCommunication):
                VpnChecksView(subtext: "Testing Tunnel Provider communication")
            case .tunnelProviderInit(_, .unexpectedError):
                VpnFailedView()
            #if os(macOS)
                case .askToRegisterLoginItem(let value):
                    RegisterLoginItemView(value: value)
            #endif
            case .ready:
                AppIcon()
                Text("Ready to launch")
            }
        }
    }
}

func AppIcon() -> some View {
    return Image(uxImage: UXImage(named: "AppIcon") ?? UXImage())
        .resizable()
        .frame(width: 64, height: 64)
}

struct VpnChecksView: View {
    var manager: NETunnelProviderManager?
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

struct VpnEnableView: View {
    var manager: NETunnelProviderManager
    var subtext: String
    var tpInit: TunnelProviderInit

    var body: some View {
        ZStack(alignment: .topLeading) {
            Image(systemName: "network.badge.shield.half.filled")
                .font(.system(size: 48))
                .foregroundStyle(.blue)
                .buttonStyle(.plain)
        }
        .padding()

        Text("Enable Obscura VPN")
            .font(.title)
        if !self.subtext.isEmpty {
            Text(self.subtext)
                .padding()
                .italic()
                .frame(width: 350)
                .frame(minHeight: 100)
                .multilineTextAlignment(.center)
        }
        Button(action: { self.tpInit.continueAfterStopOtherTunnelPriming(self.manager) }) {
            Text("Continue")
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
            Text("Please try restarting your device or contact support for help.")
        }
    }
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
            #if os(macOS)
                guard let () = await self.stepNetworkExtensionInit() else {
                    return
                }
            #endif

            guard let (tunnelProviderManager, status) = await self.stepTunnelProviderInit() else {
                return
            }

            #if os(macOS)
                await self.stepRegisterLoginItem()
            #endif

            self.update(status: .ready)
            self.appState = AppState(tunnelProviderManager, initialStatus: status)
        }
    }

    @MainActor private func update(status: StartupStatus) {
        logger.info("StartupModel.status = \(debugFormat(status), privacy: .public)")
        self.status = status
    }

    #if os(macOS)
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
    #endif

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

    #if os(macOS)
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
    #endif
}
