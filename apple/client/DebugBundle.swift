#if os(macOS)
import AppKit
import Foundation
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "DebugBundle")

struct TaskResult: Encodable {
    var total_s: Float
    var error: String?
}

/// A tool to track and manage individual bundle tasks.
private class BundleTask {
    let bundle: DebugBundleBuilder
    let name: String

    private var lock = NSLock()
    private var task: Task<Void, Never>?

    private var start = SuspendingClock.now
    private var lastPing: SuspendingClock.Instant
    private var done: SuspendingClock.Instant?

    private var timeout = Duration.seconds(60)

    /// Create and start the task.
    @discardableResult
    init(
        _ bundle: DebugBundleBuilder,
        _ name: String,
        _ f: @escaping (BundleTask) async throws -> Void
    ) {
        self.bundle = bundle
        self.name = name

        self.lastPing = self.start

        bundle.pendingTasks.start()

        self.lock.withLock {
            self.task = Task.detached(priority: .userInitiated) { [self] in
                do {
                    try await f(self)
                    self.writeResult(error: nil)
                } catch {
                    self.writeResult(error: error.localizedDescription)
                }
            }
        }

        self.watchdog()
    }

    /// Ping the watchdog timer.
    ///
    /// Throws if the task was cancelled (for example due to a timeout).
    func pingWatchdog() throws {
        try Task.checkCancellation()

        self.lastPing = SuspendingClock.now
    }

    private func watchdog() {
        self.lock.withLock {
            let deadline = self.lastPing + self.timeout
            let now = SuspendingClock.now
            if now > deadline {
                self.writeResultWithLock(error: "Timeout")
                self.task!.cancel()
            } else {
                let remaining_s = (deadline - now) / .seconds(1)
                DispatchQueue.main.asyncAfter(deadline: .now() + remaining_s) {
                    self.watchdog()
                }
            }
        }
    }

    private func writeResultWithLock(error: String?) {
        if self.done != nil { return }

        let done = SuspendingClock.now
        self.done = done

        let duration = done - self.start

        logger.info("Task \(self.name, privacy: .public) finished in \(duration, privacy: .public) error: \(error ?? "-", privacy: .public)")

        self.bundle.lock.withLock {
            self.bundle.tasks[self.name] = TaskResult(
                total_s: Float(duration / .seconds(1)),
                error: error
            )
        }

        self.bundle.pendingTasks.complete()
    }

    private func writeResult(error: String?) {
        self.lock.withLock {
            self.writeResultWithLock(error: error)
        }
    }
}

private class DebugBundleBuilder {
    let tmpFolder: URL
    let archiveFolder: URL
    let bundleTimestamp = Date()
    let jsonEncoder = JSONEncoder()
    let logStartTimestamp: Date
    let appState: AppState?

    let dispatchQueue = DispatchQueue.global(qos: .userInitiated)
    var lock = NSLock()
    var pendingTasks = PendingTasks()
    var tasks: [String: TaskResult] = [:]

    init(appState: AppState?) throws {
        self.appState = appState
        self.tmpFolder = try FileManager.default.url(
            for: FileManager.SearchPathDirectory.itemReplacementDirectory,
            in: FileManager.SearchPathDomainMask.userDomainMask,
            appropriateFor: FileManager.default.temporaryDirectory,
            create: true
        )
        self.archiveFolder = self.tmpFolder.appending(
            component: "Obscura Debugging Archive \(utcDateFormat.string(from: self.bundleTimestamp))"
        )
        try FileManager.default.createDirectory(
            at: self.archiveFolder,
            withIntermediateDirectories: false
        )

        self.jsonEncoder.outputFormatting = [
            .prettyPrinted,
            .sortedKeys,
        ]

        let uptime = ProcessInfo.processInfo.systemUptime
        self.logStartTimestamp = if uptime < 24 * 3600 {
            // If booted for less than 24h get all logs since boot.
            self.bundleTimestamp - uptime - 10 * 60
        } else {
            // Otherwise just got back 12h.
            self.bundleTimestamp - 12 * 3600
        }
    }

    deinit {
        do {
            try FileManager.default.removeItem(at: self.tmpFolder)
        } catch {
            logger.error("Error cleaning up debug bundle temp files \(error, privacy: .public)")
        }
    }

    private func createDir(name: String) throws -> URL {
        let path = self.archiveFolder.appending(component: name)
        try FileManager.default.createDirectory(at: path, withIntermediateDirectories: false)
        return path
    }

    private func copyFile(source: URL, name: String) throws {
        let path = self.archiveFolder.appending(component: name)
        try FileManager.default.copyItem(at: source, to: path)
    }

    private func openFile(name: String) throws -> FileHandle {
        let path = self.archiveFolder.appending(component: name)
        FileManager.default.createFile(atPath: path.path, contents: nil, attributes: nil)
        return try FileHandle(forWritingTo: path)
    }

    private func writeFile(name: String, data: Data) throws {
        let path = self.archiveFolder.appending(component: name)
        try data.write(to: path)
    }

    func writeJson<T>(name: String, _ json: T) throws where T: Encodable {
        let data = try self.jsonEncoder.encode(json)
        try self.writeFile(name: name, data: data)
    }

    private func writeFile(name: String, string: String) throws {
        // Safe to unwrap because String is unicode.
        let data = string.data(using: .utf8)!

        try self.writeFile(name: name, data: data)
    }

    func writeError(name: String, error: Error) {
        logger.error("Error bundling \(name, privacy: .public): \(error, privacy: .public)")
        do {
            try self.writeFile(name: "bundle-error-\(name).txt", string: error.localizedDescription)
        } catch {
            logger.error("Error bundling error for \(name, privacy: .public): \(error, privacy: .public)")
        }
    }

    private func bundleCmd(_ name: String, _ args: [String]) {
        self.bundleTask(name) { _task in
            let child = Process()
            child.executableURL = URL(filePath: args[0])
            child.arguments = Array(args.suffix(from: 1))
            child.standardInput = FileHandle.nullDevice
            child.standardOutput = try self.openFile(name: "\(name)-stdout.txt")
            child.standardError = try self.openFile(name: "\(name)-stderr.txt")

            try child.run()
            child.waitUntilExit()

            if child.terminationStatus != 0 {
                try self.writeFile(name: "\(name)-status.txt", string: String(child.terminationStatus))
            }
        }
    }

    private func bundlePlist(name: String, path: URL) {
        self.bundleTask(name) { _task in
            let plist = try Data(contentsOf: path)
            var value = try PropertyListSerialization.propertyList(from: plist, options: [], format: nil)
            prepareForJson(&value)
            let json = try JSONSerialization.data(
                withJSONObject: value,
                options: [.fragmentsAllowed, .prettyPrinted]
            )
            try self.writeFile(name: "\(name).json", data: json)
        }
    }

    private func bundlePlist(path: URL) {
        self.bundlePlist(name: path.lastPathComponent, path: path)
    }

    func bundleInfo() throws {
        struct Info: Encodable {
            let AppVersion = sourceVersion()
            let BuildNumber = buildVersion()
            let BundleTimestamp: String
            let LogStartTimestamp: String
            let LowPowerMode = ProcessInfo.processInfo.isLowPowerModeEnabled
            let macOSVersion = [
                ProcessInfo.processInfo.operatingSystemVersion.majorVersion,
                ProcessInfo.processInfo.operatingSystemVersion.minorVersion,
                ProcessInfo.processInfo.operatingSystemVersion.patchVersion,
            ]
            let macOSVersionString = ProcessInfo.processInfo.operatingSystemVersionString
            let Model = Sysctl.model // Model identifier to model name: https://support.apple.com/en-ca/102869
            let PID = ProcessInfo.processInfo.processIdentifier
            let ProcessName = ProcessInfo.processInfo.processName
            let ProcessorCountActive = ProcessInfo.processInfo.processorCount
            let ProcessorCountPhysical = ProcessInfo.processInfo.activeProcessorCount
            let ProcessorName = try? Sysctl.string(for: "machdep.cpu.brand_string") ?? "Unknown"
            let RAMPhysicalGiB = Double(ProcessInfo.processInfo.physicalMemory) / 1024.0 / 1024.0 / 1024.0
            let SourceID = sourceId()
            let ThemralState: String
            let UptimeHours = ProcessInfo.processInfo.systemUptime / 3600

            init(_ this: DebugBundleBuilder) {
                self.BundleTimestamp = utcDateFormat.string(from: this.bundleTimestamp)
                self.LogStartTimestamp = utcDateFormat.string(from: this.logStartTimestamp)
                self.ThemralState = switch ProcessInfo.processInfo.thermalState {
                case .nominal: "nominal"
                case .fair: "fair"
                case .serious: "serious"
                case .critical: "critical"
                default: "unknown"
                }
            }
        }

        try self.writeJson(name: "info.json", Info(self))
    }

    func bundleLogs() {
        self.bundleTask("logs") { task in
            let fileName: String
            let logStore: OSLogStore
            do {
                logStore = try OSLogStore.local()
                fileName = "system-log.json"
            } catch {
                self.writeError(name: "system-logs", error: error)
                logStore = try OSLogStore(scope: .currentProcessIdentifier)
                fileName = "client-log.json"
            }

            let logEntries = try logStore.getEntries(
                at: logStore.position(date: self.logStartTimestamp),
                matching: NSPredicate(format: """
                    process IN {
                        "Obscura VPN (Debug Dev Server)",
                        "Obscura VPN (Debug)",
                        "Obscura VPN",
                        "kernel",
                        "neagent",
                        "nehelper",
                        "nesessionmanager",
                        "net.obscura.vpn-client-app.system-network-extension",
                        "sysextd" }
                    || subsystem IN {
                        "com.apple.networkextension",
                        "com.apple.powerd" }
                    || eventMessage CONTAINS "bscura"
                    || subsystem CONTAINS "bscura"
                """)
            )

            let dateFormatter = DateFormatter()
            dateFormatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSSxx"

            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .formatted(dateFormatter)

            let file = try self.openFile(name: fileName)
            let newline = "\n".data(using: .utf8)!
            for entry in logEntries {
                if entry.date > self.bundleTimestamp {
                    break
                }

                var line = try encoder.encode(entry)
                line.append(newline)
                try file.write(contentsOf: line)

                try task.pingWatchdog()
            }
            try file.close()
        }
    }

    func bundleExtensions() async throws {
        let extensions = await getExtensionDebugInfo()

        struct ExtensionDebugInfo: Encodable {
            let bundleIdentifier: String
            let bundleVersion: String
            let bundleShortVersion: String
            let url: URL
            let isAwaitingUserApproval: Bool
            let isEnabled: Bool
            let isUninstalling: Bool
        }

        try self.writeJson(
            name: "extensions.json",
            extensions.map {
                ExtensionDebugInfo(
                    bundleIdentifier: $0.bundleIdentifier,
                    bundleVersion: $0.bundleVersion,
                    bundleShortVersion: $0.bundleShortVersion,
                    url: $0.url,
                    isAwaitingUserApproval: $0.isAwaitingUserApproval,
                    isEnabled: $0.isEnabled,
                    isUninstalling: $0.isUninstalling
                )
            }
        )

        for (i, ext) in extensions.enumerated() {
            if !ext.isEnabled { continue }

            let name = "extension-\(ext.bundleIdentifier)-\(i).provisionprofile"
            do {
                try self.copyFile(
                    source: ext.url.appending(path: "Contents/embedded.provisionprofile"),
                    name: name
                )
            } catch {
                self.writeError(name: name, error: error)
            }
        }
    }

    func bundleNETunnelProviderManager() {
        guard let manager = self.appState?.manager else {
            self.writeError(name: "ne-tunnel-provider-manager", error: "appState or manager is nil")
            return
        }

        struct ConnectionInfo: Encodable {
            let status: NEVPNStatus

            init(_ connection: NEVPNConnection) {
                self.status = connection.status
            }
        }
        struct ProxyServerInfo: Encodable {
            let address: String
            let authenticationRequired: Bool
            let port: Int

            init(_ proxyServer: NEProxyServer) {
                self.address = proxyServer.address
                self.authenticationRequired = proxyServer.authenticationRequired
                self.port = proxyServer.port
            }
        }
        struct ProxySettingsInfo: Encodable {
            let autoProxyConfigurationEnabled: Bool
            let exceptionList: [String]?
            let excludeSimpleHostnames: Bool
            let httpEnabled: Bool
            let httpServer: ProxyServerInfo?
            let matchDomains: [String]?
            let proxyAutoConfigurationJavaScript: String?
            let proxyAutoConfigurationURL: URL?

            init(_ proxySettings: NEProxySettings) {
                self.autoProxyConfigurationEnabled = proxySettings.autoProxyConfigurationEnabled
                self.exceptionList = proxySettings.exceptionList
                self.excludeSimpleHostnames = proxySettings.excludeSimpleHostnames
                self.httpEnabled = proxySettings.httpEnabled
                self.httpServer = proxySettings.httpServer.map { ProxyServerInfo($0) }
                self.matchDomains = proxySettings.matchDomains
                self.proxyAutoConfigurationJavaScript = proxySettings.proxyAutoConfigurationJavaScript
                self.proxyAutoConfigurationURL = proxySettings.proxyAutoConfigurationURL
            }
        }
        struct ProtocolConfigurationInfo: Encodable {
            let disconnectOnSleep: Bool
            let enforceRoutes: Bool
            let excludeLocalNetworks: Bool
            let includeAllNetworks: Bool
            let proxySettings: ProxySettingsInfo?
            let serverAddress: String?

            init(_ protocolConfiguration: NEVPNProtocol) {
                self.disconnectOnSleep = protocolConfiguration.disconnectOnSleep
                self.enforceRoutes = protocolConfiguration.enforceRoutes
                // TODO: include once our minimal version is macOS 13.3
                // self.excludeAPNs = protocolConfiguration.excludeAPNs
                // self.excludeCellularServices = protocolConfiguration.excludeCellularServices
                // self.excludeDeviceCommunication = protocolConfiguration.excludeDeviceCommunication
                self.excludeLocalNetworks = protocolConfiguration.excludeLocalNetworks
                self.includeAllNetworks = protocolConfiguration.includeAllNetworks
                self.proxySettings = protocolConfiguration.proxySettings.map { ProxySettingsInfo($0) }
                self.serverAddress = protocolConfiguration.serverAddress
            }
        }
        struct OnDemandRuleInfo: Encodable {
            let action: String
            let dnsSearchDomainMatch: [String]?
            let dnsServerAddressMatch: [String]?
            let interfaceTypeMatch: String
            let probeURL: URL?
            let ssidMatch: [String]?
            init(_ onDemandRule: NEOnDemandRule) {
                self.action = switch onDemandRule.action {
                case .connect:
                    "connect"
                case .disconnect:
                    "disconnect"
                case .evaluateConnection:
                    "evaluateConnection"
                case .ignore:
                    "ignore"
                @unknown default:
                    "unknown"
                }
                self.dnsSearchDomainMatch = onDemandRule.dnsSearchDomainMatch
                self.dnsServerAddressMatch = onDemandRule.dnsServerAddressMatch
                self.interfaceTypeMatch = switch onDemandRule.interfaceTypeMatch {
                case .any:
                    "any"
                case .ethernet:
                    "ethernet"
                case .wiFi: "wiFi"
                case .cellular: "cellular"
                @unknown default:
                    "unknown"
                }
                self.probeURL = onDemandRule.probeURL
                self.ssidMatch = onDemandRule.ssidMatch
            }
        }
        struct ManagerInfo: Encodable {
            let connection: ConnectionInfo
            let protocolConfiguration: ProtocolConfigurationInfo?
            let routingMethod: String
            let isEnabled: Bool
            let isOnDemandEnabled: Bool
            let onDemandRules: [OnDemandRuleInfo]?

            init(_ manager: NETunnelProviderManager) {
                self.connection = ConnectionInfo(manager.connection)
                self.protocolConfiguration = manager.protocolConfiguration.map { ProtocolConfigurationInfo($0) }
                self.routingMethod = switch manager.routingMethod {
                case .destinationIP:
                    "destinationIP"
                case .networkRule:
                    "networkRule"
                case .sourceApplication:
                    "sourceApplication"
                @unknown default:
                    "unknown"
                }
                self.isEnabled = manager.isEnabled
                self.isOnDemandEnabled = manager.isOnDemandEnabled
                self.onDemandRules = manager.onDemandRules.map { $0.map { OnDemandRuleInfo($0) }}
            }
        }

        do {
            try self.writeJson(name: "ne-tunnel-provider-manager.json", ManagerInfo(manager))
        } catch {
            self.writeError(name: "ne-tunnel-provider-manager", error: error)
        }
    }

    func bundleNEDebugInfo() async {
        guard let manager = self.appState?.manager else {
            self.writeError(name: "ne-debug-info", error: "appState or manager is nil")
            return
        }
        do {
            let neDebugInfoJsonString = try await runNeJsonCommand(manager, NeManagerCmd.getDebugInfo.json(), attemptTimeout: .seconds(10))
            let value = try JSONSerialization.jsonObject(with: Data(neDebugInfoJsonString.utf8))
            let json = try JSONSerialization.data(
                withJSONObject: value,
                options: [.fragmentsAllowed, .prettyPrinted, .sortedKeys]
            )
            try self.writeFile(name: "ne-debug-info.json", data: json)
        } catch {
            self.writeError(name: "ne-debug-info", error: error)
        }
    }

    func bundleTask(_ name: String, _ block: @escaping (BundleTask) async throws -> Void) {
        BundleTask(self, name, block)
    }

    func bundleAll() async {
        self.bundleLogs()

        self.bundleTask("app-provisionprofile") { _task in
            try self.copyFile(
                source: Bundle.main.bundleURL.appending(path: "Contents/embedded.provisionprofile"),
                name: "app.provisionprofile"
            )
        }

        self.bundleTask("app-extension-provisionprofile") { _task in
            try self.copyFile(
                source: extensionBundle()
                    .bundleURL
                    .appending(path: "Contents/embedded.provisionprofile"),
                name: "app-extension.provisionprofile"
            )
        }

        self.bundleTask("extensions") { _task in try await self.bundleExtensions() }
        self.bundleTask("ne-tunnel-provider-manager") { _task in self.bundleNETunnelProviderManager() }
        self.bundleTask("ne-debug-info") { _task in await self.bundleNEDebugInfo() }
        self.bundleTask("info") { _task in try self.bundleInfo() }

        self.bundleCmd("arp", ["/usr/sbin/arp", "-na"])
        self.bundleCmd("csrutil-status", ["/usr/bin/csrutil", "status"])
        self.bundleCmd("dig-apple.com", ["/usr/bin/dig", "+time=2", "www.apple.com"])
        self.bundleCmd("dig-google.com", ["/usr/bin/dig", "+time=2", "google.com"])
        self.bundleCmd("dig-v1.api.prod.obscura.net", ["/usr/bin/dig", "+time=2", "v1.api.prod.obscura.net"])
        self.bundleCmd("dns", ["/usr/sbin/scutil", "--dns", "-dv"])
        self.bundleCmd("hostinfo", ["/usr/bin/hostinfo"])
        self.bundleCmd("http-v1.api.prod.obscura.net", ["/usr/bin/curl", "--verbose", "--insecure", "--location", "https://v1.api.prod.obscura.net/api/ping"])
        self.bundleCmd("ifconfig", ["/sbin/ifconfig", "-aLbmrvv"])
        self.bundleCmd("netstat-interface-stats", ["/usr/sbin/netstat", "-ind"])
        self.bundleCmd("netstat-listen-queues", ["/usr/sbin/netstat", "-Lanv"])
        self.bundleCmd("netstat-routes", ["/usr/sbin/netstat", "-nral"])
        self.bundleCmd("netstat-stats", ["/usr/sbin/netstat", "-s"])
        self.bundleCmd("network-info", ["/usr/sbin/scutil", "--nwi", "-dv"])
        self.bundleCmd("ping-1.1.1.1", ["/sbin/ping", "-oc5", "1.1.1.1"])
        self.bundleCmd("ping-2001:4860:4860::8888", ["/sbin/ping6", "-oc5", "2001:4860:4860::8888"])
        self.bundleCmd("ping-2606:4700:4700::1111", ["/sbin/ping6", "-oc5", "2606:4700:4700::1111"])
        self.bundleCmd("ping-8.8.8.8", ["/sbin/ping", "-oc5", "8.8.8.8"])
        self.bundleCmd("ping-v1.api.prod.obscura.net", ["/sbin/ping", "-oc5", "v1.api.prod.obscura.net"])
        self.bundleCmd("processes", ["/bin/ps", "axlww"])
        self.bundleCmd("proxy", ["/usr/sbin/scutil", "--proxy", "-dv"])
        self.bundleCmd("reachability-0.0.0.0", ["/usr/sbin/scutil", "-r", "www.apple.com", "-dv"])
        self.bundleCmd("reachability-1.1.1.1", ["/usr/sbin/scutil", "-r", "1.1.1.1", "-dv"])
        self.bundleCmd("reachability-169.254.0.0", ["/usr/sbin/scutil", "-r", "169.254.0.0", "-dv"])
        self.bundleCmd("reachability-169.254.0.0", ["/usr/sbin/scutil", "-r", "169.254.0.0", "-dv"])
        self.bundleCmd("reachability-8.8.8.8", ["/usr/sbin/scutil", "-r", "8.8.8.8", "-dv"])
        self.bundleCmd("route-0.0.0.0", ["/sbin/route", "-nv", "get", "0.0.0.0"])
        self.bundleCmd("route-1.1.1.1", ["/sbin/route", "-nv", "get", "1.1.1.1"])
        self.bundleCmd("route-2001:4860:4860::8888", ["/sbin/route", "-nv", "get", "-inet6", "2001:4860:4860::8888"])
        self.bundleCmd("route-2606:4700:4700::1111", ["/sbin/route", "-nv", "get", "-inet6", "2606:4700:4700::1111"])
        self.bundleCmd("route-8.8.8.8", ["/sbin/route", "-nv", "get", "8.8.8.8"])
        self.bundleCmd("route-::", ["/sbin/route", "-nv", "get", "-inet6", "::"])
        self.bundleCmd("route-apple.com", ["/sbin/route", "-nv", "get", "www.apple.com"])
        self.bundleCmd("route-google.com", ["/sbin/route", "-nv", "get", "google.com"])
        self.bundleCmd("route-v1.api.prod.obscura.net", ["/sbin/route", "-nv", "get", "v1.api.prod.obscura.net"])
        self.bundleCmd("scutil-advisory", ["/usr/sbin/scutil", "--advisory", ""])
        self.bundleCmd("scutil-rank", ["/usr/sbin/scutil", "--rank", ""])
        self.bundleCmd("skywalk-status", ["/usr/sbin/skywalkctl", "status"])
        self.bundleCmd("sysctl", ["/usr/sbin/sysctl", "-a"])
        self.bundleCmd("vpn-connections", ["/usr/sbin/scutil", "--nc", "list"])

        self.bundlePlist(path: URL(filePath: "/Library/Preferences/SystemConfiguration/NetworkInterfaces.plist"))
        self.bundlePlist(path: URL(filePath: "/Library/Preferences/com.apple.networkd.plist"))
        self.bundlePlist(path: URL(filePath: "/Library/Preferences/com.apple.networkextension.cache.plist"))
        self.bundlePlist(path: URL(filePath: "/Library/Preferences/com.apple.networkextension.control.plist"))
        self.bundlePlist(path: URL(filePath: "/Library/Preferences/com.apple.networkextension.necp.plist"))
        self.bundlePlist(path: URL(filePath: "/etc/bootpd.plist"))

        await self.pendingTasks.waitForAll()

        do {
            try self.lock.withLock {
                try self.writeJson(name: "tasks.json", self.tasks)
            }
        } catch {
            self.writeError(name: "tasks-json", error: error)
        }
    }

    func createArchive() throws -> URL {
        let zipName = "Obscura Debuging Archive \(utcDateFormat.string(from: self.bundleTimestamp)).zip"

        var zipPath: URL?
        var coordinatorError: NSError?
        var blockError: Error?

        NSFileCoordinator().coordinate(
            readingItemAt: self.archiveFolder,
            options: [.forUploading],
            error: &coordinatorError
        ) { inUrl in
            do {
                let outDir = try FileManager.default.url(
                    for: .itemReplacementDirectory,
                    in: .userDomainMask,
                    appropriateFor: inUrl,
                    create: true
                )
                let outUrl = outDir.appendingPathComponent(zipName)

                try FileManager.default.moveItem(at: inUrl, to: outUrl)

                zipPath = outUrl
            } catch {
                blockError = error
            }
        }

        if let error = coordinatorError {
            throw error
        }
        if let error = blockError {
            throw error
        }
        guard let zipPath = zipPath else {
            throw "Archive callback never ran."
        }

        return zipPath
    }
}
#endif

public class DebugBundleStatus: Encodable {
    var inProgressCounter: Int = 0
    var inProgress: Bool {
        return self.inProgressCounter > 0
    }

    var latestPath: String?

    func start() {
        self.inProgressCounter += 1
    }

    func finish() {
        self.inProgressCounter -= 1
    }

    func setPath(_ path: String) {
        self.latestPath = path
    }

    func markError() {
        self.latestPath = nil
    }

    enum CodingKeys: String, CodingKey {
        case inProgressCounter
        case inProgress
        case latestPath
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(self.inProgressCounter, forKey: .inProgressCounter)
        try container.encode(self.inProgress, forKey: .inProgress)
        try container.encode(self.latestPath, forKey: .latestPath)
    }
}

#if os(macOS)
// Abstract DebugBundleStatus manager which ensures that inProgressCounter is appropriately incremented/decremented
public class DebugBundleRC {
    private let appState: AppState

    init(_ appState: AppState) {
        self.appState = appState

        _ = self.appState.osStatus.update { value in
            value.debugBundleStatus.start()
        }
    }

    deinit {
        _ = self.appState.osStatus.update { value in
            value.debugBundleStatus.finish()
        }
    }
}

func _createDebuggingArchive(appState: AppState?) async throws -> String {
    let _activity = ProcessInfo.processInfo.beginActivity(
        options: [
            .automaticTerminationDisabled,
            .idleSystemSleepDisabled,
            .suddenTerminationDisabled,
            .userInitiated,
        ],
        reason: "Generating Debug Bundle"
    )

    var start = SuspendingClock.now

    let builder = try DebugBundleBuilder(appState: appState)
    await builder.bundleAll()
    let zipPath = try builder.createArchive()

    let elapsed = SuspendingClock.now - start
    logger.info("Debug Bundle completed in \(elapsed, privacy: .public)")

    NSWorkspace.shared.selectFile(zipPath.path, inFileViewerRootedAtPath: "")
    return zipPath.path
}

func createDebuggingArchive(appState: AppState?) async throws -> String {
    // ensure deinit occurs at the end of the method
    let _debugBundleRc: DebugBundleRC?
    if let appState = appState {
        _debugBundleRc = DebugBundleRC(appState)
    }
    do {
        let path = try await _createDebuggingArchive(appState: appState)
        _ = appState?.osStatus.update { value in
            value.debugBundleStatus.setPath(path)
        }
        return path
    } catch {
        _ = appState?.osStatus.update { value in
            value.debugBundleStatus.markError()
        }
        throw error
    }
}
#endif
