import Combine
import libobscuravpn_client
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "network extension")

class PacketTunnelProvider: NEPacketTunnelProvider {
    weak static var shared: PacketTunnelProvider?

    private let providerId = genTaskId()
    private let isActive = AsyncMutex(false)
    private let isConnected = WatchableValue(false)
    private let networkConfig: AsyncMutex<NetworkConfig?> = AsyncMutex(.none)

    override init() {
        logger.log("init entry \(self.providerId, privacy: .public)")

        if let other = Self.shared {
            logger.warning("Multiple live PacketTunnelProvider instances. me: \(self.providerId, privacy: .public) other: \(other.providerId, privacy: .public)")
        }

        let configDir = configDir()
        let oldConfigDir = oldConfigDir()
        let userAgent = "obscura.net/macos/" + sourceVersion()
        logger.log("config dir \(configDir, privacy: .public)")
        logger.log("legacy config dir \(oldConfigDir, privacy: .public)")
        logger.log("user agent \(userAgent, privacy: .public)")
        ffiInitialize(configDir: configDir, oldConfigDir: oldConfigDir, userAgent: userAgent, receiveCallback)

        super.init()

        Self.shared = self
        self.startSendLoop()
        self.startStatusLoop()
        logger.log("init exit \(self.providerId, privacy: .public)")
    }

    deinit {
        logger.log("PacketTunnelProvider.deinit \(self.providerId, privacy: .public)")
        /*
         Hack to avoid macos bugs where handleAppMessage isn't called after deinit.
         One way to reproduce the issue:
         - disable network access (e.g. turn off wifi)
         - start a tunnel
         - any IPC that should result in handleAppMessage getting called will fail.
         This is not redundant with the `exit` in stopTunnel, because in the case described above `stopTunnel` is not called.
         */
        exit(0)
    }

    override func startTunnel(options: [String: NSObject]?) async throws {
        logger.log("startTunnel entry \(self.providerId, privacy: .public)")

        if options?.keys.contains("dontStartTunnel") == .some(true) {
            logger.critical("startTunnel \(self.providerId, privacy: .public) throws due to \"dontStartTunnel\" key in options")
            throw "dummy start with \"dontStartTunnel\" flag"
        }

        let tunnelArgs: TunnelArgs
        switch options {
        case .some(let options):
            guard let args = options["tunnelArgs"] as? String else {
                logger.critical("startTunnel \(self.providerId, privacy: .public) throws because \"tunnelArgs\" missing from options or not a string")
                throw "\"tunnelArgs\" missing from options or not a string"
            }
            tunnelArgs = try TunnelArgs(json: args)
        case .none:
            logger.info("startTunnel \(self.providerId, privacy: .public) called without options, using default tunnel args")
            tunnelArgs = TunnelArgs()
        }

        try await self.isActive.withLock { isActiveGuard in
            if isActiveGuard.value {
                logger.error("startTunnel called on active tunnel \(self.providerId, privacy: .public)")
                throw "tunnel already active"
            }

            let networkConfig = NetworkConfig(ipv4: "10.75.76.77", dns: ["10.64.0.99"], ipv6: "fc00:bbbb:bbbb:bb01::c:4c4d/128")
            try await self.setTunnelNetworkSettings(NEPacketTunnelNetworkSettings.build(networkConfig))
            let _: Empty = try await runManagerCmd(.setTunnelArgs(args: tunnelArgs))

            logger.log("set tunnel active flag \(self.providerId, privacy: .public)")
            isActiveGuard.value = true
        }

        // macos 14 cancels the tunnel if it stays on connecting for too long
        if #available(macOS 15, *) {
            logger.log("waiting for tunnel to start \(self.providerId, privacy: .public)")
            _ = await self.isConnected.waitUntil { $0 == true }
        }

        logger.log("startTunnel exit \(self.providerId, privacy: .public)")
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.log("stopTunnel entry \(self.providerId, privacy: .public)")
        await self.isActive.withLock { isActiveGuard in
            if !isActiveGuard.value {
                logger.warning("stopTunnel called on inactive tunnel \(self.providerId, privacy: .public)")
            }
            logger.log("unset tunnel active flag \(self.providerId, privacy: .public)")
            isActiveGuard.value = false

            logger.log("stopping tunnel \(self.providerId, privacy: .public)")
            do {
                let _: Empty = try await runManagerCmd(.setTunnelArgs(args: .none))
            } catch {
                logger.critical("setting empty tunnel args failed: \(error, privacy: .public)")
            }
        }
        logger.log("waiting for tunnel to stop \(self.providerId, privacy: .public)")
        _ = await self.isConnected.waitUntil { $0 == false }
        logger.log("stopTunnel exit and abort \(self.providerId, privacy: .public)")
        /*
         Hack to avoid macos bugs where no methods of self are called after stopTunnel including deinit.
         */
        exit(0)
    }

    override func handleAppMessage(_ msg: Data, completionHandler: ((Data?) -> Void)?) {
        guard let completionHandler = completionHandler else {
            logger.error("received app message without completion handler")
            return
        }
        guard let isSetTunnelArgs = try? [String: Empty].self.init(json: msg).keys.contains("setTunnelArgs") else {
            logger.error("received invalid app message, expected JSON encoded manager command")
            let json_error = try! NeManagerCmdResult.error(errorCodeOther).json().data(using: .utf8)
            completionHandler(json_error)
            return
        }
        Task {
            let isActiveGuard = isSetTunnelArgs ? await self.isActive.lock() : nil
            defer { withExtendedLifetime(isActiveGuard) {}}
            if isActiveGuard?.value == .some(false) {
                // Prevent accidental tunnel starts due to a "setTunnelArgs" command that lost a race with "stopTunnel".
                let json_error = try! NeManagerCmdResult.error(errorCodeTunnelInactive).json().data(using: .utf8)
                completionHandler(json_error)
            } else {
                let json_result = try! await ffiJsonManagerCmd(msg).json()
                completionHandler(json_result.data(using: .utf8))
            }
        }
    }

    override func sleep() async {
        logger.log("sleep entry \(self.providerId, privacy: .public)")
        // TODO: eagerly stop tunnel that is going to die anyway
        logger.log("sleep exit \(self.providerId, privacy: .public)")
    }

    override func wake() {
        logger.log("wake entry \(self.providerId, privacy: .public)")
        // TODO: start tunnel, see TODO in sleep, relying on quic connection keepalive for now
        // TODO: Try to wait for network connection to come back. Otherwise first attempt is guaranteed to fail on wifi, which adds delay.
        logger.log("wake exit \(self.providerId, privacy: .public)")
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
                logger.error("Send task for deallocated PacketTunnelProvider \(providerId, privacy: .public) called")
                return
            }
            if providerId != Self.shared?.providerId {
                logger.error("Send task for obsolete PacketTunnelProvider \(providerId, privacy: .public) called")
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
            logger.log("status loop entry \(taskId, privacy: .public)")

            var knownVersion: UUID? = .none
            while true {
                let status = await getRustStatus(knownVersion: knownVersion)
                knownVersion = status.version
                guard let self = self else {
                    logger.error("status loop for deallocated PacketTunnelProvider \(providerId, privacy: .public) exiting")
                    break
                }
                await self.processStatusUpdate(status)
            }
            logger.log("status loop exit \(taskId, privacy: .public)")
        }
    }

    func processStatusUpdate(_ status: NeStatus) async {
        logger.log("processing status update \(status.version, privacy: .public)")
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
            case .connected(_, _, let networkConfig, _, _):
                if isActiveGuard.value {
                    do {
                        try await self.ensureNetworkConfig(newNetworkConfig: networkConfig)
                        self.reasserting = false
                    } catch {
                        logger.critical("setting network config failed \(error, privacy: .public)")
                    }
                }
            }
        }
        logger.log("finished processing status update \(status.version, privacy: .public)")
    }

    func ensureNetworkConfig(newNetworkConfig: NetworkConfig) async throws {
        try await self.networkConfig.withLock { networkConfigGuard in
            // This check isn't needed for correctness, but skipping unnecessary calls to `setTunnelNetworkSettings` does prevent brief periods with packet loss and lot of OS activity visible in the system log.
            if networkConfigGuard.value != newNetworkConfig {
                logger.info("setting network config \(newNetworkConfig, privacy: .public)")
                let networkSettings = NEPacketTunnelNetworkSettings.build(newNetworkConfig)
                try await self.setTunnelNetworkSettings(networkSettings)
                networkConfigGuard.value = newNetworkConfig
            } else {
                logger.info("keeping existing network config \(newNetworkConfig, privacy: .public)")
            }
        }
    }
}

private func receiveCallback(packet: FfiBytes) {
    guard let inst = PacketTunnelProvider.shared else {
        logger.error("Packet callback called with no active PacketTunnelProvider")
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
            logger.critical("error getting rust status \(error, privacy: .public)")
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
