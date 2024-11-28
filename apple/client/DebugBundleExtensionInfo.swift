import Foundation
import NetworkExtension
import OSLog
import SystemExtensions

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "DebugBundleExtensionInfo")

func getExtensionDebugInfo() async -> [OSSystemExtensionProperties] {
    var delegate: Delegate? // OSSystemExtensionManager doesn't keep our delegate alive, so we need to take a reference.

    return await withCheckedContinuation { continuation in
        let request = OSSystemExtensionRequest.propertiesRequest(
            forExtensionWithIdentifier: extensionBundleID(),
            queue: .main
        )
        delegate = Delegate(continuation)
        request.delegate = delegate
        OSSystemExtensionManager.shared.submitRequest(request)
    }
}

private class Delegate: NSObject {
    let continuation: CheckedContinuation<[OSSystemExtensionProperties], Never>

    init(_ continuation: CheckedContinuation<[OSSystemExtensionProperties], Never>) {
        self.continuation = continuation
    }
}

extension Delegate: OSSystemExtensionRequestDelegate {
    func request(
        _ request: OSSystemExtensionRequest,
        actionForReplacingExtension existing: OSSystemExtensionProperties,
        withExtension ext: OSSystemExtensionProperties
    ) -> OSSystemExtensionRequest.ReplacementAction {
        return .cancel
    }

    func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {}

    func request(
        _ request: OSSystemExtensionRequest,
        didFinishWithResult result: OSSystemExtensionRequest.Result
    ) {}

    func request(
        _ request: OSSystemExtensionRequest,
        didFailWithError error: any Error
    ) {}

    func request(
        _ request: OSSystemExtensionRequest,
        foundProperties extensions: [OSSystemExtensionProperties]
    ) {
        self.continuation.resume(returning: extensions)
    }
}
