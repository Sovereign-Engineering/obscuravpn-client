import NetworkExtension
import OSLog
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "SubscriptionManageViewModel")

@MainActor
final class SubscriptionManageViewModel: ObservableObject {
    @ObservedObject var appState: AppState

    @Published var accountInfo: AccountInfo?
    @Published var isLoading = false
    @Published var initialLoad = true
    @Published var showErrorAlert = false

    // Bool is canRepeat
    private var refreshTaskPerCanRepeat: [Bool: Task<Void, Error>?] = [:]

    // If true StoreKit thinks the subscription is owned but server does not show it as owned
    // If you want to observe this property you must have both storeKitModel and SubscriptionManageViewModel as observed
    var storeKitPurchasedAwaitingServerAck: Bool {
        let hasStoreKitSubscription = self.appState.storeKitModel.subscribed
        let hasServerSubscription = self.accountInfo?.appleSubscription?.subscriptionStatus == .active

        // Return true if StoreKit shows active but server doesn't yet
        return hasStoreKitSubscription && !hasServerSubscription
    }

    init(appState: AppState, accountInfo: AccountInfo? = nil) {
        self.appState = appState
        self.accountInfo = accountInfo
        self.initialLoad = accountInfo == nil

        Task {
            await self.refresh(repeatWithBinaryBackoffAllowed: true, userOriginated: true)
        }
    }

    var displayPrice: String? {
        guard let subscriptionProduct = self.appState.storeKitModel.subscriptionProduct else {
            return nil
        }
        guard let period = subscriptionProduct.subscriptionPeriodFormatted() else {
            return nil
        }
        return "\(subscriptionProduct.displayPrice)/\n\(period)"
    }

    func loadAccountInfo(showLoading: Bool = true) async {
        if showLoading {
            self.isLoading = true
        }
        do {
            let accountInfo = try await appState.getAccountInfo()
            self.accountInfo = accountInfo
            if showLoading {
                self.isLoading = false
            }
            self.initialLoad = false
        } catch {
            logger.error("Failed to load account info: \(error, privacy: .public)")
            self.showErrorAlert = true
            if showLoading {
                self.isLoading = false
            }
            self.initialLoad = false
        }
    }

    func purchaseSubscription() async throws {
        guard self.accountInfo != nil else {
            logger.error("can't purchase unless logged in")
            return
        }

        do {
            let purchased = try await self.appState.purchaseSubscription()
            if purchased {
                await self.afterPurchase()
            }
        } catch {
            logger.error("purchase failed: \(error, privacy: .public)")
            throw error
        }
    }

    func afterPurchase() async {
        await self.refresh(
            repeatWithBinaryBackoffAllowed: true,
            userOriginated: false
        )
    }

    func refresh(repeatWithBinaryBackoffAllowed: Bool = false, userOriginated: Bool = false) async {
        self.refreshTaskPerCanRepeat[repeatWithBinaryBackoffAllowed]??.cancel()
        self.refreshTaskPerCanRepeat[repeatWithBinaryBackoffAllowed] = Task {
            await self.appState.storeKitModel.updatePurchases()

            // Do one simple load to see if we can get a match between client and server state
            await self.loadAccountInfo(showLoading: userOriginated)

            if self.storeKitPurchasedAwaitingServerAck {
                if repeatWithBinaryBackoffAllowed {
                    try await self.pollCheckingForServerAcknoledgementOfSubscription()
                } else {
                    await self.loadAccountInfo(showLoading: false)
                }
            }

            do {
                let _ = try await self.appState.associateAccount()
            } catch {
                logger.warning("Failed to associate Apple account on refresh: \(error, privacy: .public)")
            }
        }
        try? await self.refreshTaskPerCanRepeat[repeatWithBinaryBackoffAllowed]??.value
    }

    // Polls with binary backoff checking for updates to loadAccountInfo until
    // client and server paid state match up. Periodically, if they do not match up,
    // calls askBackendToCheckTransactionId to attempt to force backend to
    // match it up
    private func pollCheckingForServerAcknoledgementOfSubscription() async throws {
        let delays = [1, 2, 4, 8, 16, 32, 64]

        for delay in delays {
            logger.debug("Polling loadAccountInfo, then send backend transaction id, then wait \(delay)")
            await self.loadAccountInfo(showLoading: false)

            // If that load resolved the mismatch return
            if !self.storeKitPurchasedAwaitingServerAck {
                return
            }
            await self.loadAccountInfo(showLoading: false)

            // Schedule a loadAccount info 3 seconds after poll subscription always so we hear of a change without waiting for next backoff
            Task {
                try await Task.sleep(for: .seconds(3))
                await self.loadAccountInfo(showLoading: false)
            }

            try await Task.sleep(for: .seconds(delay))
        }

        logger.error("Server failed to acknowledge subscription after all retries")
        self.showErrorAlert = true
    }
}
