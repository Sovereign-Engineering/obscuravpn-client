// Most of the code in this file is in a temporary state.
// Clean up tracked by this ticket https://linear.app/soveng/issue/OBS-1672/cleanup-temporary-ios-code

import NetworkExtension
import SwiftUI

enum TempStartupStatus {
    case initial
    case tunnelProviderInit(TunnelProviderInit, TunnelProviderInitStatus)
    case ready

    var description: String {
        switch self {
        case .initial:
            return "TempStartupStatus.initial"
        case let .tunnelProviderInit(providerInit, status):
            return "TempStartupStatus.tunnelProviderInit \(status)"
        case .ready:
            return "TempStartupStatus.ready"
        }
    }
}

// A replacement for StartupModel
class TemporaryiOSStartupModel: ObservableObject {
    @Published var appState: AppState?
    @Published var status = TempStartupStatus.initial

    public func start() {
        Task { @MainActor in
            // Note: Skipped stepNetworkExtensionInit

            guard let (tunnelProviderManager, status) = await self.stepTunnelProviderInit() else {
                return
            }

            // Note: Skipped stepRegisterLoginItem no place in iOS

            self.update(status: .ready)
            self.appState = AppState(tunnelProviderManager, initialStatus: status)
        }
    }

    @MainActor private func update(status: TempStartupStatus) {
        self.status = status
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
        return nil
    }
}

@MainActor
class TempStatusPoller: ObservableObject {
    @Published var managerCount: Int = 0
    @Published var tunnelProviderErrorString: String = ""
    @Published var managerStatus: NEVPNStatus? = nil

    init() {
        Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { _ in
            NETunnelProviderManager.loadAllFromPreferences { managers, errors in
                Task { @MainActor in
                    self.managerCount = managers?.count ?? 0
                    self.tunnelProviderErrorString = errors?.localizedDescription ?? "None"
                    self.managerStatus = managers?.first?.connection.status
                }
            }
        }
    }
}

struct IOSClientAppDevView: View {
    @ObservedObject var startupModel: TemporaryiOSStartupModel = .init()
    @ObservedObject var temporaryUpdated = TempStatusPoller()
    @SceneStorage("accountId") var accountId: String = ""

    @FocusState private var textFieldFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            self.controls
            if let appState = startupModel.appState {
                ContentView(appState: appState)
            }
            Spacer()
            self.status
        }
        .padding()
    }

    var controls: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Controls")
            Button("StartupModel.start") {
                self.startupModel.start()
            }
            Button("Continue after priming") {
                if case let .tunnelProviderInit(tunnelProviderInit, _) = startupModel.status {
                    tunnelProviderInit.continueAfterPermissionPriming()
                }
            }
            TextField("ID", text: self.$accountId)
                .padding()
                .focused(self.$textFieldFocused)
                .onTapGesture {
                    self.textFieldFocused = true
                }
            Button("Login") {
                if let manager = startupModel.appState?.manager {
                    Task {
                        _ = try? await neLogin(
                            manager,
                            accountId: self.accountId,
                            attemptTimeout: .seconds(10),
                            maxAttempts: 3
                        )
                    }
                }
            }
            .disabled(self.startupModel.appState?.manager == nil)
            Button("Enable Tunnel") {
                Task {
                    do {
                        try await self.startupModel.appState?.enableTunnel(TunnelArgs(exit: .any))
                    } catch {
                        print("Failed to connect \(error)")
                    }
                }
            }
        }
    }

    var status: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Dev View")
                .font(.title)
            Group {
                Text("StartupModel.status \(self.startupModel.status)")
                    .font(.body)
                Text("NETunnelProviderManager Count \(self.temporaryUpdated.managerCount)")
                Text("NETunnelProviderManager errors \(self.temporaryUpdated.tunnelProviderErrorString)")
                Text("appState.status.accountId \(self.startupModel.appState?.status.accountId ?? "nil")")
                Text("appState.status.account \(self.startupModel.appState?.status.account)")
                Text("appState?.manager.connection.status \(self.temporaryUpdated.managerStatus)")
            }
            .font(.body)
        }
    }
}

@main
struct MyExampleApp: App {
    var body: some Scene {
        WindowGroup {
            IOSClientAppDevView()
        }
    }
}
