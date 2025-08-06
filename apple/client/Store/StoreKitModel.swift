import Foundation
import NetworkExtension
import os
import StoreKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StoreKitModel")

// Important per https://developer.apple.com/documentation/storekit/transaction/updates
// "If your app has unfinished transactions, the updates listener receives them once, immediately after the app launches. Without the Task to listen for these transactions, your app may miss them."
// IE Get this object created ASAP!!!!!!
class StoreKitModel: ObservableObject {
    @Published @MainActor private(set) var products: [Product] = []
    @Published @MainActor private(set) var purchasedProducts: [Product] = []
    private var updateListenerTask: Task<Void, Error>?
    private var purchaseIntentListenerTask: Task<Void, Error>?
    private let manager: NETunnelProviderManager?

    init(manager: NETunnelProviderManager?) {
        self.manager = manager
        self.purchaseIntentListenerTask = self.listenForPurchaseIntents()
        self.updateListenerTask = self.listenForTransactions()

        if manager == nil {
            logger.info("Warning!! Without a NE Manager there is no way to complete purchases")
        }
        Task {
            try? await self.reloadProductsAvailable()
            await self.updateStoreKitSubscriptionStatus()
        }
    }

    deinit {
        updateListenerTask?.cancel()
        purchaseIntentListenerTask?.cancel()
    }

    func reloadProductsAvailable() async throws {
        logger.info("Attempting to reload products reloadProductsAvailable")

        Task { @MainActor in
            do {
                self.products = try await Product
                    .products(for: ObscuraProduct.allProductIds)
                logger.info("Loaded \(self.products.count) products")
            } catch {
                logger.error("Failed to load products: \(error)")
            }
        }
    }

    func purchase(_ product: Product, accountId: String) async throws -> Bool {
        guard let appAccountToken = try await appAccountToken(accountId: accountId) else {
            throw "Could not purchase product. Could not fetch appAccountToken for \(accountId)"
        }
        do {
            let options: Set<Product.PurchaseOption> = [.appAccountToken(appAccountToken)]
            let result = try await product.purchase(options: options)

            switch result {
            case .success(let verification):
                // Always finish a transaction.
                if case .verified(let transaction) = verification {
                    await transaction.finish()
                    if self.isTransactionRelevant(transaction) {
                        return true
                    } else {
                        logger.error("Verification of successful purchase failed")
                    }
                }
            case .userCancelled:
                logger.info("Purchase canceled by user")
            case .pending:
                // may succeed in the future get updates via Transaction.updates
                logger.info("Purchase is pending")
            @unknown default:
                logger.error("Purchase encountered unknown error")
            }
        } catch {
            logger.error("Purchase failed: \(error.localizedDescription)")
            throw error
        }
        return false
        // TODO: Interrupted purchase? https://developer.apple.com/documentation/storekit/testing-an-interrupted-purchase
    }

    func restorePurchases() async {
        do {
            try await AppStore.sync()
            await self.updateStoreKitSubscriptionStatus()
        } catch {
            logger.error("Failed to restore purchases")
        }
    }

    // MARK: - Private

    private func isTransactionRelevant(_ transaction: Transaction) -> Bool {
        if let revocationDate = transaction.revocationDate {
            logger.error("Transaction verification failed: Transaction was revoked on \(revocationDate)")
            return false
        }
        if let expirationDate = transaction.expirationDate,
           expirationDate < Date()
        {
            logger.error("Transaction verification failed: Transaction expired on \(expirationDate)")
            return false
        }
        if transaction.isUpgraded {
            logger.error("Transaction verification failed: Transaction was upgraded")
            return false
        }
        logger.info("Transaction verification succeeded")
        return true
    }

    private func listenForPurchaseIntents() -> Task<Void, Error>? {
        guard let manager else {
            logger.error("Cannot listen for purchase intents without a NE Manager")
            return nil
        }
        logger.info("Began listening for Purchase Intents")
        return Task.detached {
            for await purchaseIntent in PurchaseIntent.intents {
                do {
                    logger
                        .info(
                            "Received purchase intent for \(purchaseIntent.product.displayName) need to fetch accountID"
                        )
                    let accountId = try await getAccountInfo(
                        manager
                    ).id
                    _ = try await self.purchase(
                        purchaseIntent.product,
                        accountId: accountId
                    )
                } catch {
                    assertionFailure()
                    logger.error("Purchase intent purchase failed")
                }
            }
        }
    }

    private func listenForTransactions() -> Task<Void, Error> {
        logger.info("Began listening for Transaction.updates")
        return Task.detached {
            for await result in Transaction.updates {
                await self.updateStoreKitSubscriptionStatus()

                // Always finish a transaction.
                if case .verified(let transaction) = result {
                    await transaction.finish()
                }
            }
        }
    }

    @MainActor public func updateStoreKitSubscriptionStatus() async {
        var currentProductsPurchased: [Product] = []

        for await result in Transaction.currentEntitlements {
            if case .verified(let transaction) = result {
                if self.isTransactionRelevant(transaction) {
                    if let product = products.first(
                        where: { $0.id == transaction.productID
                        })
                    {
                        currentProductsPurchased.append(product)
                    }
                }

                await transaction.finish()
            }
        }

        logger
            .info(
                "updateStoreKitSubscriptionStatus read all of Transaction.currentEntitlements and got (\(currentProductsPurchased.count)) \(currentProductsPurchased.map(\.displayName).joined(separator: ","))"
            )
        self.purchasedProducts = currentProductsPurchased
    }

    func appAccountToken(accountId: String) async throws -> UUID? {
        guard let manager else {
            return nil
        }
        let persistedTokenMappings = PersistedAppAccountTokenMappings()

        // Get appAccountToken
        let appAccountToken: UUID
        if let existingToken = persistedTokenMappings.getAccountToken(
            for: accountId)
        {
            appAccountToken = existingToken
        } else {
            do {
                appAccountToken = try await neApiAppleCreateAppAccountToken(
                    manager
                ).appAccountToken
                persistedTokenMappings
                    .setAccountToken(
                        accountId: accountId,
                        appAccountToken: appAccountToken
                    )
            } catch {
                logger.error("Failed to get app account token: \(error)")
                throw error
            }
        }
        return appAccountToken
    }

    @MainActor public func isPurchased(product: Product) async -> Bool {
        guard let state = await product.currentEntitlement else {
            return false
        }

        switch state {
        case .unverified:
            return false
        case .verified(let transaction):
            await transaction.finish()
            return self.isTransactionRelevant(transaction)
        }
    }

    // In theory this should always match isPurchased
    // However by computing based on published purchased products this makes cachedIsPurchased observable
    @MainActor public func cachedIsPurchased(product: Product) -> Bool {
        return self.purchasedProducts.contains(where: { $0.id == product.id })
    }

    @MainActor public func transactionId(product: Product) async -> UInt64? {
        guard let state = await product.currentEntitlement else {
            return nil
        }

        switch state {
        case .unverified:
            return nil
        case .verified(let transaction):
            await transaction.finish()
            return self.isTransactionRelevant(
                transaction
            ) ? transaction.id : nil
        }
    }
}

// MARK: - Convenience

extension StoreKitModel {
    enum ObscuraProduct: String, CaseIterable {
        case monthlySubscription = "subscriptions.monthly"

        static var allProductIds: [String] {
            ObscuraProduct.allCases.map(\.rawValue)
        }
    }

    @MainActor func product(for product: ObscuraProduct) -> Product? {
        self.products.first { $0.id == product.rawValue }
    }

    @MainActor func availableStoreKitProductObject(_ product: ObscuraProduct) -> Product? {
        self.products.first { $0.id == product.rawValue }
    }

    func purchase(obscuraProduct: ObscuraProduct, accountId: String) async throws -> Bool {
        guard let availableProduct = await availableStoreKitProductObject(
            obscuraProduct
        ) else {
            throw "Cannot purchase \(obscuraProduct) as it is not in the list of StoreKit available products"
        }
        return try await self.purchase(availableProduct, accountId: accountId)
    }

    @MainActor var hasActiveMonthlySubscription: Bool {
        guard let subscriptionProduct = product(for: .monthlySubscription) else {
            logger.error("Could not get subscription product")
            return false
        }
        return self.purchasedProducts
            .contains(where: { $0.id == subscriptionProduct.id })
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
