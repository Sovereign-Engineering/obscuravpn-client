import os
import StoreKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StoreKitModel")

@MainActor class StoreKitModel: ObservableObject {
    @Published private var products: [Product] = []
    @Published private var purchasedProducts: [Product] = []
    @Published var renewalPrice: String? = nil

    private let subscriptionProductId = "subscriptions.monthly"
    var subscriptionProduct: Product? {
        return self.products.first { $0.id == self.subscriptionProductId }
    }

    var subscribed: Bool {
        return self.purchasedProducts.contains(where: { $0.id == self.subscriptionProductId })
    }

    @Published private var storefront: Storefront?
    var externalPaymentsAllowed: Bool {
        // External payments are currently only straightforward in the US.
        return self.storefront?.countryCode == "USA"
    }

    nonisolated init() {
        Task { @MainActor in
            await self.updateStorefront(await Storefront.current)
        }
    }

    func updateStorefront(_ storefront: Storefront?) async {
        self.storefront = storefront
        do {
            self.products = try await Product.products(for: [self.subscriptionProductId])
        } catch {
            logger.error("failed to load products: \(error, privacy: .public)")
        }
        await self.updatePurchases()
    }

    func updatePurchases() async {
        self.purchasedProducts.removeAll()
        self.renewalPrice = nil
        // For auto-renewable subscriptions, `currentEntitlements` only contains
        // the latest non-expired transaction.
        for await result in Transaction.currentEntitlements {
            if case .verified(let transaction) = result {
                if let product = products.first(where: { $0.id == transaction.productID }) {
                    self.purchasedProducts.append(product)
                    if product.id == self.subscriptionProductId,
                       let subscription = product.subscription
                    {
                        do {
                            for status in try await subscription.status {
                                if case .verified(let renewalInfo) = status.renewalInfo {
                                    if let renewalPrice = renewalInfo.renewalPrice,
                                       let renewalCurrency = renewalInfo.currency
                                    {
                                        self.renewalPrice = renewalPrice.formatted(.currency(code: renewalCurrency.identifier))
                                    }
                                }
                            }
                        } catch {
                            logger.error("Failed to fetch subscription renewal info: \(error, privacy: .public)")
                        }
                        break
                    }
                }
            }
        }
    }

    func restorePurchases() async throws(String) {
        do {
            try await AppStore.sync()
            await self.updatePurchases()
        } catch {
            logger.error("failed to restore purchases: \(error, privacy: .public)")
            throw "failed to restore purchases: \(error)"
        }
    }

    // This is here just so we can keep `products` completely private.
    func collectDebugData() async throws -> [Any] {
        var debugData: [Any] = []
        for product in self.products {
            var subscriptionStatus: [[String: String]] = []
            if let subscription = product.subscription {
                for status in try await subscription.status {
                    subscriptionStatus.append(["state": status.state.localizedDescription])
                }
            }
            try debugData.append([
                "product": JSONSerialization.jsonObject(with: product.jsonRepresentation),
                "subscriptionStatus": subscriptionStatus,
            ])
        }
        return debugData
    }

    func toSubscriptionModel() -> SubscriptionProductModel? {
        if let subscriptionProduct = self.subscriptionProduct {
            return SubscriptionProductModel(
                displayName: subscriptionProduct.displayName,
                description: subscriptionProduct.description,
                displayPrice: subscriptionProduct.displayPrice,
                renewalPrice: self.renewalPrice,
                subscriptionPeriodFormatted: subscriptionProduct.subscriptionPeriodFormatted()
            )
        }
        return nil
    }
}

// static representation of useful information derived from a StoreKit Product
class SubscriptionProductModel: Codable {
    var displayName: String
    var description: String
    var displayPrice: String
    var renewalPrice: String?
    var subscriptionPeriodFormatted: String?
    init(displayName: String, description: String, displayPrice: String, renewalPrice: String?, subscriptionPeriodFormatted: String? = nil) {
        self.displayName = displayName
        self.description = description
        self.displayPrice = displayPrice
        self.renewalPrice = renewalPrice
        self.subscriptionPeriodFormatted = subscriptionPeriodFormatted
    }
}
