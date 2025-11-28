import os
import StoreKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StoreKitListener")

/// Apple wants us to start listening "as soon as your app launches":
/// https://developer.apple.com/documentation/storekit/transaction/updates
class StoreKitListener {
    private let purchaseIntentsListener: Task<Void, Error>
    private let transactionUpdatesListener: Task<Void, Error>
    private let storefrontUpdatesListener: Task<Void, Error>

    init(appState: AppState) {
        self.purchaseIntentsListener = Task.detached {
            for await purchaseIntent in PurchaseIntent.intents {
                do {
                    _ = try await appState.purchase(product: purchaseIntent.product)
                } catch {
                    logger.error("failed to honor purchase intent: \(error, privacy: .public)")
                }
            }
        }
        self.transactionUpdatesListener = Task.detached {
            // `updates` is for transactions that happen outside the app or on
            // other devices, and also receives queued unfinished transactions
            // once at launch.
            for await result in Transaction.updates {
                if case .verified(let transaction) = result {
                    // We don't really have a concept of "undelivered"
                    // transactions, so if any transactions are somehow left
                    // unfinished we should just mark them as finished.
                    await transaction.finish()
                }
                await appState.storeKitModel.updatePurchases()
            }
        }
        self.storefrontUpdatesListener = Task.detached {
            // "The storefront value can change at any time."
            // https://developer.apple.com/documentation/storekit/storefront/updates
            for await storefront in Storefront.updates {
                await appState.storeKitModel.updateStorefront(storefront)
            }
        }
    }

    deinit {
        self.purchaseIntentsListener.cancel()
        self.transactionUpdatesListener.cancel()
        self.storefrontUpdatesListener.cancel()
    }
}
