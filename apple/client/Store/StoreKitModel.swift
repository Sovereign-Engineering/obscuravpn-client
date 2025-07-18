import Foundation
import os
import StoreKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StoreKitModel")

// Important per https://developer.apple.com/documentation/storekit/transaction/updates
// "If your app has unfinished transactions, the updates listener receives them once, immediately after the app launches. Without the Task to listen for these transactions, your app may miss them."
// IE Get this object created ASAP!!!!!!
class StoreKitModel: ObservableObject {
    @Published @MainActor private(set) var productsAvailable: [Product] = []
    @Published @MainActor private(set) var productsPurchased: [Product] = []
    private var updateListenerTask: Task<Void, Error>?

    init() {
        self.updateListenerTask = self.listenForTransactions()

        Task {
            try? await self.reloadProductsAvailable()
            await self.updateStoreKitSubscriptionStatus()
        }
    }

    deinit {
        updateListenerTask?.cancel()
    }

    func reloadProductsAvailable() async throws {
        guard await self.productsAvailable.isEmpty else { return }

        Task { @MainActor in
            self.productsAvailable = try await Product
                .products(for: ObscuraProduct.allProductIds)
        }
    }

    func purchase(_ product: Product, appAccountToken: UUID? = nil) async throws {
        do {
            let options: Set<Product.PurchaseOption>
            if let token = appAccountToken {
                options = [.appAccountToken(token)]
            } else {
                options = []
            }
            let result = try await product.purchase(options: options)

            switch result {
            case .success(let verification):
                // Always finish a transaction.
                if case .verified(let transaction) = verification {
                    await transaction.finish()
                }

                if let _ = verifyTransactionRelevant(verification) {
                    await self.updateStoreKitSubscriptionStatus()
                } else {
                    logger.error("Verification of successful purchase failed")
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

    @MainActor func isPurchased(productId: String) -> Bool {
        return self.productsPurchased.contains(where: { $0.id == productId })
    }

    // MARK: - Private

    private func verifyTransactionRelevant(_ result: VerificationResult<Transaction>) -> Transaction? {
        switch result {
        case .unverified:
            logger.error("Transaction verification failed: Transaction is unverified")
            return nil
        case .verified(let transaction):
            if let revocationDate = transaction.revocationDate {
                logger.error("Transaction verification failed: Transaction was revoked on \(revocationDate)")
                return nil
            }
            if let expirationDate = transaction.expirationDate,
               expirationDate < Date()
            {
                logger.error("Transaction verification failed: Transaction expired on \(expirationDate)")
                return nil
            }
            if transaction.isUpgraded {
                logger.error("Transaction verification failed: Transaction was upgraded")
                return nil
            }
            logger.info("Transaction verification succeeded")
            return transaction
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

    @MainActor private func updateStoreKitSubscriptionStatus() async {
        var currentProductsPurchased: [Product] = []

        for await result in Transaction.currentEntitlements {
            if let verifiedTransaction = self.verifyTransactionRelevant(result) {
                if let product = productsAvailable.first(
                    where: { $0.id == verifiedTransaction.productID
                    })
                {
                    currentProductsPurchased.append(product)
                }
            }

            // Always finish a transaction.
            if case .verified(let transaction) = result {
                await transaction.finish()
            }
        }

        logger
            .info(
                "updateStoreKitSubscriptionStatus read all of Transaction.currentEntitlements and got (\(currentProductsPurchased.count)) \(currentProductsPurchased.map(\.displayName).joined(separator: ","))"
            )
        self.productsPurchased = currentProductsPurchased
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
        self.productsAvailable.first { $0.id == product.rawValue }
    }

    @MainActor func availableStoreKitProductObject(_ product: ObscuraProduct) -> Product? {
        self.productsAvailable.first { $0.id == product.rawValue }
    }

    func purchase(obscuraProduct: ObscuraProduct, appAccountToken: UUID? = nil) async throws {
        guard let availableProduct = await availableStoreKitProductObject(
            obscuraProduct
        ) else {
            throw "Cannot purchase \(obscuraProduct) as it is not in the list of StoreKit available products"
        }
        try await self.purchase(availableProduct, appAccountToken: appAccountToken)
    }

    @MainActor var hasActiveMonthlySubscription: Bool {
        self.productsPurchased.contains { $0.id == ObscuraProduct.monthlySubscription.rawValue }
    }
}
