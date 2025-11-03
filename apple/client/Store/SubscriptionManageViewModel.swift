import NetworkExtension
import OSLog
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "SubscriptionManageViewModel")

@MainActor
final class SubscriptionManageViewModel: ObservableObject {
    private let manager: NETunnelProviderManager?
    @ObservedObject var storeKitModel: StoreKitModel

    @Published var accountInfo: AccountInfo?
    @Published var isLoading = false
    @Published var initialLoad = true
    @Published var showErrorAlert = false
    @Published var debugGestureActivated = false

    // Bool is canRepeat
    private var refreshTaskPerCanRepeat: [Bool: Task<Void, Error>?] = [:]

    // If true StoreKit thinks the subscription is owned but server does not show it as owned
    // If you want to observe this property you must have both storeKitModel and SubscriptionManageViewModel as observed
    var storeKitPurchasedAwaitingServerAck: Bool {
        let hasStoreKitSubscription = self.storeKitModel.hasActiveMonthlySubscription
        let hasServerSubscription = self.accountInfo?.appleSubscription?.subscriptionStatus == .active

        // Return true if StoreKit shows active but server doesn't yet
        return hasStoreKitSubscription && !hasServerSubscription
    }

    init(manager: NETunnelProviderManager?, storeKitModel: StoreKitModel? = nil, accountInfo: AccountInfo? = nil) {
        self.manager = manager
        self.storeKitModel = storeKitModel ?? StoreKitModel(manager: manager)
        self.accountInfo = accountInfo
        self.initialLoad = accountInfo == nil

        Task {
            await self.refresh(repeatWithBinaryBackoffAllowed: true, userOriginated: true)
        }
    }

    var monthlySubscriptionProduct: Product? {
        self.storeKitModel.product(for: .monthlySubscription)
    }

    var displayPrice: String? {
        if let monthlySubscriptionProduct, let subscriptionPeriodFormatted = monthlySubscriptionProduct.subscriptionPeriodFormatted() {
            return "\(monthlySubscriptionProduct.displayPrice)/\n\(subscriptionPeriodFormatted)"
        }
        return nil
    }

    func loadAccountInfo(showLoading: Bool = true) async {
        guard let manager = manager else {
            logger.warning("Attempted to load account info without manager")
            return
        }

        if showLoading {
            self.isLoading = true
        }
        do {
            let accountInfo = try await getAccountInfo(manager)
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

    func restorePurchases() async {
        await self.storeKitModel.restorePurchases()
    }

    func purchaseSubscription() async throws {
        guard let id = self.accountInfo?.id else {
            logger.error("Cannot purchaseSubscription without a manager or id")
            return
        }

        // Purchase
        do {
            let purchased = try await self.storeKitModel
                .purchase(
                    obscuraProduct: .monthlySubscription,
                    accountId: id
                )
            if purchased {
                await self.afterAnyPurchase()
            }
        } catch {
            logger.error("Purchase failed: \(error)")
            throw error
        }
    }

    func onOfferCodeRedemption() async {
        await self.afterAnyPurchase()
    }

    private func afterAnyPurchase() async {
        await self.refresh(
            repeatWithBinaryBackoffAllowed: true,
            userOriginated: false
        )
    }

    func refresh(repeatWithBinaryBackoffAllowed: Bool = false, userOriginated: Bool = false) async {
        if userOriginated {
            Task {
                await self.logCurrentStatus(context: "User originated refresh")
            }
        }
        self.refreshTaskPerCanRepeat[repeatWithBinaryBackoffAllowed]??.cancel()
        self.refreshTaskPerCanRepeat[repeatWithBinaryBackoffAllowed] = Task {
            await self.storeKitModel.updateStoreKitSubscriptionStatus()

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
                try await self.storeKitModel.associateAccount()
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

        Task {
            await self.logCurrentStatus(context: "Starting to poll in pollCheckingForServerAcknoledgementOfSubscription")
        }

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
        Task {
            await self.logCurrentStatus(context: "polling pollCheckingForServerAcknoledgementOfSubscription failed")
        }
        self.showErrorAlert = true
    }

    private func logCurrentStatus(context: String) async {
        let originalTransactionIdString: String
        if let monthlySubscriptionProduct {
            if let originalTransactionId = await storeKitModel.originalTransactionId(
                product: monthlySubscriptionProduct)
            {
                originalTransactionIdString = String(originalTransactionId)
            } else {
                originalTransactionIdString = "(nil)"
            }
        } else {
            originalTransactionIdString = "(nil PROBLEM we do not have monthlySubscriptionProduct)"
        }

        logger.log("SubscriptionViewModel Status Update: \"\(context)\" accountId: \(self.accountInfo?.id ?? "(nil)"), purchasedSubscription: \(self.storeKitModel.hasActiveMonthlySubscription), originalTransactionId: \(originalTransactionIdString), accountInfo: \(self.accountInfo?.description ?? "(nil)") ")
    }
}
