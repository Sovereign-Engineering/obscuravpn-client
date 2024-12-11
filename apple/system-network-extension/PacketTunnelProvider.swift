import Combine
import libobscuravpn_client
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "network extension")

class PacketTunnelProvider: NEPacketTunnelProvider {
    weak static var shared: PacketTunnelProvider?

    private let providerId = genTaskId()

    var isActive = false

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
        ffiInitialize(configDir: configDir, oldConfigDir: oldConfigDir, userAgent: userAgent)

        super.init()

        Self.shared = self
        self.startSendLoop()
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
        self.isActive = true

        if options?.keys.contains("dontStartTunnel") == .some(true) {
            logger.critical("startTunnel \(self.providerId, privacy: .public) throws due to \"dontStartTunnel\" key in options")
            throw "dummy start with \"dontStartTunnel\" flag"
        }

        let jsonTunnelArgs: String
        switch options {
        case .some(let options):
            guard let args = options["tunnelArgs"] as? String else {
                logger.critical("startTunnel \(self.providerId, privacy: .public) throws because \"tunnelArgs\" missing from options or not a string")
                throw "\"tunnelArgs\" missing from options or not a string"
            }
            jsonTunnelArgs = args
        case .none:
            logger.info("startTunnel \(self.providerId, privacy: .public) called without options, using default tunnel args")
            do {
                jsonTunnelArgs = try TunnelArgs().json()
            } catch {
                logger.critical("startTunnel \(self.providerId, privacy: .public) throws due to JSON serialization error in default TunnelArgs \(error, privacy: .public)")
                throw "could not serialize default TunnelArgs"
            }
        }

        if case .failure(let err) = await connect(jsonTunnelArgs) {
            logger.error("startTunnel throws \(self.providerId, privacy: .public): \(err, privacy: .public)")
            throw NSError(connectErrorCode: err)
        }
        logger.log("startTunnel exit \(self.providerId, privacy: .public)")
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.log("stopTunnel entry \(self.providerId, privacy: .public)")
        self.isActive = false
        libobscuravpn_client.stop_tunnel()
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
        guard let json_cmd = String(data: msg, encoding: .utf8) else {
            logger.error("received non-UTF8 app message, excpected JSON")
            let json_err = try! NeFfiJsonResult.error(errorCodeOther).json()
            completionHandler(json_err.data(using: .utf8))
            return
        }
        Task {
            let json_result = try! await ffiJsonCmd(json_cmd).json()
            completionHandler(json_result.data(using: .utf8))
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

    func connect(_ jsonTunnelArgs: String) async -> Result<Void, String> {
        switch await ffiStartTunnel(jsonTunnelArgs, receiveCallback, networkConfigCallback, tunnelStatusCallback) {
        case .success(let ffiNetworkConfig):
            logger.log("network config: \(ffiNetworkConfig, privacy: .public)")
            let networkSettings = NEPacketTunnelNetworkSettings.build(ffiNetworkConfig)
            logger.log("NEPacketTunnelNetworkSettings: \(networkSettings, privacy: .public)")
            do {
                try await self.setTunnelNetworkSettings(networkSettings)
            } catch {
                logger.error("failed to set network settings: \(error, privacy: .public)")
                return .failure(errorCodeOther)
            }

            self.reasserting = false
            return .success(())
        case .failure(let err):
            return .failure(err)
        }
    }
}

// TODO: remove in favor of a task with a status subscription, or just stop setting reasserting
private func tunnelStatusCallback(isConnected: Bool) {
    guard let inst = PacketTunnelProvider.shared else {
        logger.error("Tunnel status callback called with no active PacketTunnelProvider")
        return
    }
    if !inst.isActive {
        logger.error("Refusing to update reasserting for inactive tunnel.")
        return
    }
    logger.log("Tunnel status callback called. isConnected: \(isConnected, privacy: .public)")
    if isConnected {
        inst.reasserting = false
    } else if #available(macOS 15, *) {
        // macos 14 disconnects the tunnel if it stays on reasserting for 5min. This problem is exacerbated by unreliable sleep. 5min time awake can accumulate in less than an hour with the lid closed.
        inst.reasserting = true
    }
}

private func networkConfigCallback(ffiNetworkConfig: FfiBytes) {
    logger.log("Network config callback called")
    guard let inst = PacketTunnelProvider.shared else {
        logger.error("Network config callback called with no active PacketTunnelProvider")
        return
    }

    if !inst.isActive {
        logger.error("Refusing to apply networking config for inactive tunnel.")
        return
    }

    guard let networkConfig = try? FfiNetworkConfig(json: ffiNetworkConfig.data()) else {
        logger.error("Failed to decode network config.")
        return
    }

    let networkSettings = NEPacketTunnelNetworkSettings.build(networkConfig)
    logger.log("NEPacketTunnelNetworkSettings: \(networkSettings, privacy: .public)")
    Task {
        let taskId = genTaskId()
        logger.log("set network settings task entry \(taskId, privacy: .public)")
        do {
            try await inst.setTunnelNetworkSettings(networkSettings)
        } catch {
            logger.critical("failed to set network settings in network config callback: \(error, privacy: .public)")
        }
        logger.log("set network settings task exit \(taskId, privacy: .public)")
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
