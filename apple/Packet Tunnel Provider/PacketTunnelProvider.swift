import Combine
import libobscuravpn_client
import NetworkExtension

class PacketTunnelProvider: NEPacketTunnelProvider {
    weak static var shared: PacketTunnelProvider?

    private let providerId = genTaskId()
    private let isActive = AsyncMutex(false)
    private let isConnected = WatchableValue(false)
    private let networkConfig: AsyncMutex<NetworkConfig?> = AsyncMutex(.none)
    private let nwPathMonitor: NWPathMonitor = .init()

    var selfObservation: NSKeyValueObservation?

    override init() {
        let logFlushGuard = libobscuravpn_client.initialize_apple_system_logging()
        ffiLog(.Info, "init entry \(self.providerId)")

        if let other = Self.shared {
            ffiLog(.Warn, "Multiple live PacketTunnelProvider instances. me: \(self.providerId) other: \(other.providerId)")
        }

        let configDir = configDir()
        #if os(macOS)
            let userAgentPlatform = "macos"
        #else
            let userAgentPlatform = "ios"
        #endif
        let userAgent = "obscura.net/" + userAgentPlatform + "/" + sourceVersion()
        ffiLog(.Info, "config dir \(configDir)")
        ffiLog(.Info, "user agent \(userAgent)")
        ffiInitialize(configDir: configDir, userAgent: userAgent, logFlushGuard: logFlushGuard, receiveCallback)

        self.nwPathMonitor.pathUpdateHandler = { path in
            if path.status != .satisfied {
                ffiLog(.Info, "network path not satisfied")
                ffiSetNetworkInterfaceIndex(.none)
                return
            }
            switch path.availableInterfaces.first {
            case .some(let preferredInterface):
                ffiLog(.Info, "preferred network path interface name: \(preferredInterface.name), index: \(preferredInterface.index)")
                ffiSetNetworkInterfaceIndex(.some(preferredInterface.index))
            case .none:
                ffiLog(.Info, "no available network path interface")
                ffiSetNetworkInterfaceIndex(.none)
            }
        }
        self.nwPathMonitor.start(queue: .main)

        super.init()

        self.selfObservation = self.observe(
            \.protocolConfiguration,
            options: [.old, .new]
        ) { [weak self] object, change in
            Task {
                await self?.handleProtocolConfigurationChange(change: change)
            }
        }

        Self.shared = self
        self.startSendLoop()
        self.startStatusLoop()
        ffiLog(.Info, "init exit \(self.providerId)")
    }

    deinit {
        ffiLog(.Info, "PacketTunnelProvider.deinit \(self.providerId)")
        /*
         Hack to avoid macos bugs where handleAppMessage isn't called after deinit.
         One way to reproduce the issue:
         - disable network access (e.g. turn off wifi)
         - start a tunnel
         - any IPC that should result in handleAppMessage getting called will fail.
         This is not redundant with the `exit` in stopTunnel, because in the case described above `stopTunnel` is not called.

         https://linear.app/soveng/issue/OBS-2070
         */
        exit(0)
    }

    override func startTunnel(options: [String: NSObject]?) async throws {
        ffiLog(.Info, "startTunnel entry \(self.providerId), includeAllNetworks: \(self.protocolConfiguration.includeAllNetworks)")

        if options?.keys.contains("dontStartTunnel") == .some(true) {
            ffiLog(.Error, "startTunnel \(self.providerId) throws due to \"dontStartTunnel\" key in options")
            throw "dummy start with \"dontStartTunnel\" flag"
        }

        let tunnelArgs: TunnelArgs
        switch options {
        case .some(let options):
            guard let args = options["tunnelArgs"] as? String else {
                ffiLog(.Error, "startTunnel \(self.providerId) throws because \"tunnelArgs\" missing from options or not a string")
                throw "\"tunnelArgs\" missing from options or not a string"
            }
            tunnelArgs = try TunnelArgs(json: args)
        case .none:
            ffiLog(.Info, "startTunnel \(self.providerId) called without options, using default tunnel args")
            tunnelArgs = TunnelArgs(
                exit: .any
            )
        }

        try await self.isActive.withLock { isActiveGuard in
            if isActiveGuard.value {
                ffiLog(.Error, "startTunnel called on active tunnel \(self.providerId)")
                throw "tunnel already active"
            }

            let networkConfig = NetworkConfig(ipv4: "10.75.76.77", dns: ["10.64.0.99"], ipv6: "fc00:bbbb:bbbb:bb01::c:4c4d/128", mtu: 1280)
            try await self.setTunnelNetworkSettings(NEPacketTunnelNetworkSettings.build(networkConfig))
            let _: Empty = try await runManagerCmd(.setTunnelArgs(args: tunnelArgs, allowActivation: true))

            ffiLog(.Info, "set tunnel active flag \(self.providerId)")
            isActiveGuard.value = true
        }

        // macos 14 cancels the tunnel if it stays on connecting for too long
        if #available(macOS 15, *) {
            ffiLog(.Info, "waiting for tunnel to start \(self.providerId)")
            _ = await self.isConnected.waitUntil { $0 == true }
        }

        ffiLog(.Info, "startTunnel exit \(self.providerId)")
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        ffiLog(.Info, "stopTunnel entry \(self.providerId)")
        await self.isActive.withLock { isActiveGuard in
            if !isActiveGuard.value {
                ffiLog(.Warn, "stopTunnel called on inactive tunnel \(self.providerId)")
            }
            ffiLog(.Info, "unset tunnel active flag \(self.providerId)")
            isActiveGuard.value = false

            ffiLog(.Info, "stopping tunnel \(self.providerId)")
            do {
                let _: Empty = try await runManagerCmd(.setTunnelArgs(args: .none, allowActivation: false))
            } catch {
                ffiLog(.Error, "setting empty tunnel args failed: \(error)")
            }
        }
        ffiLog(.Info, "waiting for tunnel to stop \(self.providerId)")
        _ = await self.isConnected.waitUntil { $0 == false }
        ffiLog(.Info, "stopTunnel exit and abort \(self.providerId)")
        /*
         Hack to avoid macos bugs where no methods of self are called after stopTunnel including deinit.

         https://linear.app/soveng/issue/OBS-2069
         */
        exit(0)
    }

    override func handleAppMessage(_ msg: Data, completionHandler: ((Data?) -> Void)?) {
        guard let completionHandler = completionHandler else {
            ffiLog(.Error, "received app message without completion handler")
            return
        }
        Task {
            let json_result = try! await ffiJsonManagerCmd(msg).json()
            completionHandler(json_result.data(using: .utf8))
        }
    }

    override func sleep() async {
        ffiLog(.Info, "sleep entry \(self.providerId)")
        ffiLog(.Info, "sleep exit \(self.providerId)")
    }

    override func wake() {
        ffiLog(.Info, "wake entry \(self.providerId)")
        ffiWake()
        ffiLog(.Info, "wake exit \(self.providerId)")
    }

    func startSendLoop() {
        /*
             Note: This code is a bit unusual for a handful of reasons.

             1. This must not keep `self` alive.
             2. `self.packetFlow.readPackets` just never calls its completion handler when this provider is obsolete. This means that we can't run any cleanup code. It also means we can't use a `Task` as it would never complete.
             3. We want to check and log if we are called after a new `PacketTunnelProvider` has been created.

             In the end we still leak the `handle` callback. But this is basically the minimum we can leak. Neither we or anyone on GitHub appears to have found a way to leak nothing with this API. We aren't the only ones to notice as I found many examples of people using a `weak self` parameter.
         */
        let providerId = self.providerId
        var handle: (([Data], [NSNumber]) -> Void)?
        handle = { [weak self] (packets: [Data], _protocols: [NSNumber]) in
            guard let self = self else {
                ffiLog(.Error, "Send task for deallocated PacketTunnelProvider \(providerId) called")
                return
            }
            if providerId != Self.shared?.providerId {
                ffiLog(.Error, "Send task for obsolete PacketTunnelProvider \(providerId) called")
                return
            }

            for packet in packets {
                packet.withFfiBytes {
                    libobscuravpn_client.send_packet($0)
                }
            }

            self.packetFlow.readPackets(completionHandler: handle!)
        }
        self.packetFlow.readPackets(completionHandler: handle!)
    }

    func startStatusLoop() {
        let providerId = self.providerId
        Task { [weak self] in
            let taskId = genTaskId()
            ffiLog(.Info, "status loop entry \(taskId)")

            var knownVersion: UUID? = .none
            while true {
                let status = await getRustStatus(knownVersion: knownVersion)
                knownVersion = status.version
                guard let self = self else {
                    ffiLog(.Error, "status loop for deallocated PacketTunnelProvider \(providerId) exiting")
                    break
                }
                await self.processStatusUpdate(status)
            }
            ffiLog(.Info, "status loop exit \(taskId)")
        }
    }

    func processStatusUpdate(_ status: NeStatus) async {
        ffiLog(.Info, "processing status update \(status.version)")
        _ = self.isConnected.update {
            $0 = switch status.vpnStatus {
            case .connected: true
            default: false
            }
        }
        await self.isActive.withLock { isActiveGuard in
            switch status.vpnStatus {
            case .disconnected:
                fallthrough
            case .connecting:
                if isActiveGuard.value {
                    // macos 14 disconnects the tunnel if it stays on reasserting for 5min. This problem is exacerbated by unreliable sleep. 5min time awake can accumulate in less than an hour with the lid closed.
                    if #available(macOS 15, *) {
                        self.reasserting = true
                    }
                }
            case .connected(_, _, let networkConfig, _, _, _):
                if isActiveGuard.value {
                    do {
                        try await self.ensureNetworkConfig(newNetworkConfig: networkConfig)
                        self.reasserting = false
                    } catch {
                        ffiLog(.Error, "setting network config failed \(error)")
                    }
                }
            }
        }
        ffiLog(.Info, "finished processing status update \(status.version)")
    }

    func ensureNetworkConfig(newNetworkConfig: NetworkConfig) async throws {
        try await self.networkConfig.withLock { networkConfigGuard in
            // This check isn't needed for correctness, but skipping unnecessary calls to `setTunnelNetworkSettings` does prevent brief periods with packet loss and lot of OS activity visible in the system log.
            if networkConfigGuard.value != newNetworkConfig {
                ffiLog(.Info, "setting network config \(newNetworkConfig)")
                let networkSettings = NEPacketTunnelNetworkSettings.build(newNetworkConfig)
                try await self.setTunnelNetworkSettings(networkSettings)
                networkConfigGuard.value = newNetworkConfig
            } else {
                ffiLog(.Info, "keeping existing network config \(newNetworkConfig)")
            }
        }
    }

    func handleProtocolConfigurationChange(change: NSKeyValueObservedChange<NEVPNProtocol>) async {
        ffiLog(.Info, "handleProtocolConfigurationChange entry \(change.oldValue) to \(change.newValue)")
        defer {
            ffiLog(.Info, "handleProtocolConfigurationChange exit")
        }

        guard let old = change.oldValue else {
            // First value, no need to react.
            return
        }

        guard let new = change.newValue else {
            ffiLog(.Warn, "protocolConfiguration changed to (null)!")
            return
        }

        guard !old.includeAllNetworks && new.includeAllNetworks else {
            ffiLog(.Info, "No interesting changes.")
            return
        }
        ffiLog(.Info, "includeAllNetorks has been enabled.")

        await self.isActive.withLock { isActiveGuard in
            if !isActiveGuard.value {
                ffiLog(.Info, "Not active, ignoring.")
                return
            }

            await self.networkConfig.withLock { networkConfigGuard in
                guard let networkConfig = networkConfigGuard.value else {
                    ffiLog(.Info, "No existing network config, doing nothing.")
                    return
                }
                ffiLog(.Info, "re-setting network config.")
                let networkSettings = NEPacketTunnelNetworkSettings.build(networkConfig)
                do {
                    try await self.setTunnelNetworkSettings(networkSettings)
                    ffiLog(.Info, "Network settings reconfigured.")
                } catch {
                    ffiLog(.Error, "Failed to apply network settings. User is probably offline \(error)")
                }
            }
        }
    }
}

private func receiveCallback(packet: FfiBytes) {
    guard let inst = PacketTunnelProvider.shared else {
        ffiLog(.Error, "Packet callback called with no active PacketTunnelProvider")
        return
    }
    let packet = packet.data()
    Task {
        inst.packetFlow.writePackets([packet], withProtocols: [NSNumber(value: AF_INET)])
    }
}

private func genTaskId() -> String {
    Data((1 ... 5).map { _ in UInt8.random(in: 65 ... 90) }).reduce("") { $0 + String(format: "%c", $1) }
}

func getRustStatus(knownVersion: UUID?) async -> NeStatus {
    while true {
        do {
            return try await runManagerCmd(.getStatus(knownVersion: knownVersion))
        } catch {
            ffiLog(.Error, "error getting rust status \(error)")
        }
        try! await Task.sleep(seconds: 1)
    }
}

func runManagerCmd<O: Codable>(_ cmd: NeManagerCmd) async throws -> O {
    let jsonCmd = try cmd.json()
    switch await ffiJsonManagerCmd(Data(jsonCmd.utf8)) {
    case .ok_json(let ok):
        return try O(json: ok)
    case .error(let err):
        throw err
    }
}
