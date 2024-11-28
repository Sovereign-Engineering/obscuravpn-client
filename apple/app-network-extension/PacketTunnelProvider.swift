import libobscuravpn_client
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "app network extension")

class PacketTunnelProvider: NEPacketTunnelProvider {
    override init() {
        logger.log("init")
        super.init()
        logger.log("config dir \(configDir(), privacy: .public)")
        libobscuravpn_client.initialize_macos_system_logging()
    }

    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        completionHandler(nil)
    }

    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        completionHandler()
    }

    override func handleAppMessage(_ messageData: Data, completionHandler: ((Data?) -> Void)?) {
        logger.log("handleAppMessage")
        if let handler = completionHandler {
            handler(nil)
        }
    }

    override func sleep(completionHandler: @escaping () -> Void) {
        completionHandler()
    }

    override func wake() {}
}
