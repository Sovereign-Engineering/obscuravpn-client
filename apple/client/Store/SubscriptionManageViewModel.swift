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
        self.initialLoad = accountInfo != nil

        // If we have a manager and no initial account info, load it
        if manager != nil && accountInfo == nil {
            Task {
                await self.loadAccountInfo()
            }
        }

        if self.storeKitPurchasedAwaitingServerAck {
            Task {
                await self.pollSubscription()
                await self.checkForServerAcknoledgementOfSubscription()
            }
            Task {
                do {
                    try await self.storeKitModel.associateAccount()
                } catch {
                    logger.warning("Failed to associate Apple account in init: \(error, privacy: .public)")
                }
            }
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
                try await Task.sleep(for: .seconds(20))
                await self.checkForServerAcknoledgementOfSubscription()
                if self.storeKitPurchasedAwaitingServerAck {
                    await self.pollSubscription()
                }
            }
        } catch {
            logger.error("Purchase failed: \(error)")
            throw error
        }
    }

    private func checkForServerAcknoledgementOfSubscription() async {
        let delays = [1, 2, 4, 8, 16, 32, 64]

        for delay in delays {
            await self.loadAccountInfo(showLoading: false)

            // If that load resolved the mismatch return
            if !self.storeKitPurchasedAwaitingServerAck {
                return
            }

            try? await Task.sleep(for: .seconds(delay))
        }

        logger.error("Server failed to acknowledge subscription after all retries")
        self.showErrorAlert = true
    }

    func refresh() async {
        await self.storeKitModel.updateStoreKitSubscriptionStatus()
        await self.loadAccountInfo()
        if self.storeKitPurchasedAwaitingServerAck {
            await self.pollSubscription()
        }
        do {
            try await self.storeKitModel.associateAccount()
        } catch {
            logger.warning("Failed to associate Apple account on refresh: \(error, privacy: .public)")
        }
    }

    private func pollSubscription() async {
        guard let manager, let monthlySubscriptionProduct, let accountId = self.accountInfo?.id,
              // Only accounts with an app account token can be polled
              let appAccountToken = try? await storeKitModel.appAccountToken(accountId: accountId),
              let originalTransactionId = await storeKitModel.originalTransactionId(
                  product: monthlySubscriptionProduct)
        else {
            return
        }

        logger.info(
            "Polling subscription with app account token \(appAccountToken, privacy: .public) and original transaction ID \(originalTransactionId, privacy: .public)"
        )

        try? await neApiApplePollSubscription(
            manager,
            originalTransactionId: String(originalTransactionId)
        )
        await self.loadAccountInfo()
    }
}

private class PersistedAppAccountTokenMappings {
    private static let userDefaultsKey = "storekit_account_token"

    func setAccountToken(accountId: String, appAccountToken: UUID) {
        let data = [accountId: appAccountToken.uuidString]
        UserDefaults.standard.set(data, forKey: Self.userDefaultsKey)
    }

    func getAccountToken(for accountId: String) -> UUID? {
        guard let data = UserDefaults.standard.dictionary(forKey: Self.userDefaultsKey) as? [String: String],
              let uuidString = data[accountId],
              data.keys.first == accountId,
              let uuid = UUID(uuidString: uuidString)
        else {
            return nil
        }
        return uuid
    }
}
