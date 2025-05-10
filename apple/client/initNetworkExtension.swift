import Foundation
import NetworkExtension
import OSLog
import SystemExtensions

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "NetworkExtensionInit")

enum NetworkExtensionInitStatus {
    case checking
    case blockingBeforePermissionPopup
    case blockingBeforeTunnelDisconnect
    case enabling
    case waitingForUserApproval

    // Terminal states.
    case failed(String)
    case waitingForReboot
}

enum NetworkExtensionInitEvent {
    case status(NetworkExtensionInitStatus)
    case done
}

class NetworkExtensionInit: NSObject {
    var continuation: AsyncStream<NetworkExtensionInitEvent>.Continuation?
    private var canceling = false
    private var activationRequested = false
    private let tunnelConnected: Bool

    init(tunnelConnected: Bool) {
        self.tunnelConnected = tunnelConnected
    }

    func start() -> AsyncStream<NetworkExtensionInitEvent> {
        logger.log("Starting NetworkExtensionInit")
        return AsyncStream<NetworkExtensionInitEvent> { continuation in
            self.continuation = continuation

            self.update(.checking)
            logger.log("Requesting system extension properties...")
            let request = OSSystemExtensionRequest.propertiesRequest(
                forExtensionWithIdentifier: networkExtensionBundleID(),
                queue: .main
            )
            request.delegate = self
            OSSystemExtensionManager.shared.submitRequest(request)
        }
    }

    func continueAfterPriming() {
        if self.activationRequested {
            logger.error("activation requested multiple times")
            return
        }
        self.activationRequested = true
        logger.log("Requesting system extension activation/replacement...")
        let request = OSSystemExtensionRequest.activationRequest(
            forExtensionWithIdentifier: networkExtensionBundleID(),
            queue: .main
        )
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }

    private func update(_ status: NetworkExtensionInitStatus) {
        logger.log("NetworkExtensionInit state: \(debugFormat(status), privacy: .public)")
        if let cont = self.continuation {
            cont.yield(.status(status))
        }
    }

    private func done() {
        logger.log("NetworkExtensionInit done")
        if let cont = self.continuation {
            cont.yield(.done)
            cont.finish()
        }
    }
}

extension NetworkExtensionInit: OSSystemExtensionRequestDelegate {
    func request(
        _ request: OSSystemExtensionRequest,
        foundProperties sysExts: [OSSystemExtensionProperties]
    ) {
        // This method will be called after we submit a `OSSystemExtensionRequest.propertiesRequest`, which happens automatically in `start()`
        logger.debug("Step 1: OSSystemExtensionRequestDelegate.request(... foundProperties ...) called")
        let buildVersion = buildVersion()
        logger.debug("matching system extension bundle version against app build version \(buildVersion, privacy: .public)")
        var matchingBundleIdAlreadyEnabled = false
        var matchingBuildVersionAlreadyEnabled = false
        for sysExt in sysExts {
            logger.debug("found system extensions \(sysExt.bundleIdentifier) \(sysExt.bundleShortVersion) \(sysExt.bundleVersion), enabled: \(sysExt.isEnabled), awaitingUserApproval: \(sysExt.isAwaitingUserApproval)")
            if sysExt.bundleIdentifier == networkExtensionBundleID() && sysExt.isEnabled {
                matchingBundleIdAlreadyEnabled = true
                if sysExt.bundleVersion == buildVersion {
                    matchingBuildVersionAlreadyEnabled = true
                }
            }
        }

        if matchingBuildVersionAlreadyEnabled {
            logger.info("found enabled system extension with matching build version, not expecting a replacement, requesting activation")
            self.continueAfterPriming()
        } else if self.tunnelConnected {
            logger.info("found connected tunnel, but the build version of the enabled system extension doesn't match, expecting tunnel disconnect, waiting for external activation trigger")
            self.update(.blockingBeforeTunnelDisconnect)
        } else if matchingBundleIdAlreadyEnabled {
            logger.info("found enabled system extension, tunnel not connected, not expecting to get blocked, requesting activation")
            self.continueAfterPriming()
        } else {
            logger.info("found no enabled system extension, expecting to get blocked, waiting for external activation trigger")
            self.update(.blockingBeforePermissionPopup)
        }
    }

    func request(
        _ request: OSSystemExtensionRequest,
        actionForReplacingExtension oldExt: OSSystemExtensionProperties,
        withExtension newExt: OSSystemExtensionProperties
    ) -> OSSystemExtensionRequest.ReplacementAction {
        // This method will be called after we submit a `OSSystemExtensionRequest.activationRequest`, which is either:
        // - automatcially triggered if we don't expect to get blocked by the OS
        // - triggered by `Self.continueAfterPriming()` being called from the outside, so the caller can prepare the user for the popup and approval steps or tunnel disconnect
        logger.debug("Step 2: OSSystemExtensionRequestDelegate.request(... actionForReplacingExtension ...) called")

        var replacementRequired = false

        let matchingBundleId = oldExt.bundleIdentifier == newExt.bundleIdentifier
        logger.debug("bundleIdentifier matches? \(matchingBundleId, privacy: .public) (\(newExt.bundleIdentifier))")
        if !matchingBundleId {
            logger.error("Unexpected bundleIdentifier old: \(oldExt.bundleIdentifier, privacy: .public)")
            replacementRequired = true
        }

        let matchingShortVersion = oldExt.bundleShortVersion == newExt.bundleShortVersion
        logger.debug("bundleShortVersion maches? \(matchingShortVersion, privacy: .public) (\(newExt.bundleShortVersion))")
        if !matchingShortVersion {
            replacementRequired = true
            logger.debug("old.bundleShortVersion: \(oldExt.bundleShortVersion, privacy: .public)")
        }

        let matchingVersion = oldExt.bundleVersion == newExt.bundleVersion
        logger.debug("bundleVersion matches? \(matchingVersion, privacy: .public) (\(newExt.bundleVersion))")
        if !matchingVersion {
            replacementRequired = true
            logger.debug("old.bundleVersion: \(oldExt.bundleVersion, privacy: .public)")
        }

        logger.log("System extension replacement required? \(replacementRequired)")
        if replacementRequired {
            self.update(.enabling)
            return .replace
        } else {
            self.canceling = true
            return .cancel
        }
    }

    func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
        logger.debug("Step 3: OSSystemExtensionRequestDelegate.requestNeedsUserApproval(...) called")
        self.update(.waitingForUserApproval)
    }

    func request(
        _ request: OSSystemExtensionRequest,
        didFinishWithResult result: OSSystemExtensionRequest.Result
    ) {
        logger.debug("Step 4: OSSystemExtensionRequestDelegate.request(... didFinishWithResult ...) called")
        switch result {
        case .completed:
            self.done()
        case .willCompleteAfterReboot:
            self.update(.waitingForReboot)
        @unknown default:
            logger.error("sys ext request unknown result variant: \(debugFormat(result), privacy: .public)")
            self.update(.failed("Unknown activation result: \(result.rawValue)"))
        }
    }

    func request(_ request: OSSystemExtensionRequest, didFailWithError error: Error) {
        logger.error("OSSystemExtensionRequestDelegate.request(... didFailWithError ...) called: \(error.localizedDescription, privacy: .public)")

        switch error {
        case let error as OSSystemExtensionError:
            switch OSSystemExtensionError.Code(rawValue: error.errorCode) {
            case .requestCanceled:
                if self.canceling {
                    logger.info("System extension installation skipped.")
                    // This should only happen for systems with system extension dev mode enabled.
                    self.done()
                } else {
                    self.update(.failed("Unexpected system extension install cancellation."))
                }
            case nil:
                self.update(.failed("Invalid error code: \(error.errorCode)"))
            default:
                self.update(.failed(error.localizedDescription))
            }
        default:
            self.update(.failed(error.localizedDescription))
        }
    }
}
