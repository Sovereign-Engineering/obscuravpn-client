import StoreKit
import SwiftUI

struct StoreDebugUI: View {
    @ObservedObject public var storeKitModel: StoreKitModel
    let accountId: String?
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var selectedProduct: Product?
    @State private var appAccountToken: UUID?

    var body: some View {
        NavigationStack {
            VStack {
                HStack {
                    Text("Store Debug")
                        .font(.title2)
                        .fontWeight(.bold)

                    if self.isLoading {
                        ProgressView()
                    }

                    Spacer()
                }

                HStack {
                    VStack {
                        Text("Account ID")
                            .fontWeight(.bold)
                        Text(self.accountId ?? "nil")
                            .textSelection(.enabled)
                            .font(.subheadline)
                    }
                    .frame(maxWidth: .infinity)

                    VStack {
                        Text("App Account Token")
                            .fontWeight(.bold)
                            .task {
                                // Load app account token on view open
                                if let accountId {
                                    appAccountToken = try? await self.storeKitModel.appAccountToken(accountId: accountId)
                                } else {
                                    appAccountToken = nil
                                }
                            }
                        if let appAccountToken {
                            Text(appAccountToken.uuidString.lowercased())
                                .textSelection(.enabled)
                                .font(.subheadline)
                        } else {
                            ProgressView()
                        }
                    }
                    .frame(maxWidth: .infinity)
                }
                .padding(.vertical)

                HStack {
                    Button("Reload Products") {
                        Task {
                            await self.reloadProducts()
                        }
                    }
                    .buttonStyle(.borderedProminent)

                    Button("Restore Purchases") {
                        Task {
                            await self.restorePurchases()
                        }
                    }
                    .buttonStyle(.borderedProminent)
                }

                if self.storeKitModel.products.isEmpty {
                    ContentUnavailableView(
                        "No Products Available",
                        systemImage: "cart.badge.questionmark",
                        description: Text("Products will appear here once they're loaded from the App Store.")
                    )
                } else {
                    ScrollView {
                        VStack {
                            ForEach(self.storeKitModel.products, id: \.id) { product in
                                ProductRow(
                                    product: product,
                                    isPurchased: self.storeKitModel.cachedIsPurchased(product: product),
                                    onPurchase: {
                                        Task {
                                            await self.purchaseProduct(product)
                                        }
                                    },
                                    onTap: {
                                        self.selectedProduct = product
                                    }
                                )
                            }
                        }
                        .padding(.horizontal)
                    }
                }

                Spacer()
            }
            .padding()
            .navigationDestination(item: self.$selectedProduct) { product in
                ProductDetailView(product: product, store: self.storeKitModel)
            }
            .alert("Error", isPresented: Binding(
                get: { self.errorMessage != nil },
                set: { if !$0 { self.errorMessage = nil } }
            )) {
                Button("OK") {
                    self.errorMessage = nil
                }
            } message: {
                if let errorMessage = errorMessage {
                    Text(errorMessage)
                }
            }
        }
    }

    private func reloadProducts() async {
        self.isLoading = true

        do {
            try await self.storeKitModel.reloadProductsAvailable()
        } catch {
            self.errorMessage = "Failed to reload available products"
        }

        self.isLoading = false
    }

    private func purchaseProduct(_ product: Product) async {
        self.isLoading = true

        guard let accountId else {
            self.errorMessage = "Cannot purchase without an accountId"
            self.isLoading = false
            return
        }

        do {
            try await self.storeKitModel.purchase(product, accountId: accountId)
        } catch {
            self.errorMessage = "Purchase failed: \(error.localizedDescription)"
        }

        self.isLoading = false
    }

    private func restorePurchases() async {
        self.isLoading = true

        await self.storeKitModel.restorePurchases()

        self.isLoading = false
    }
}

struct ProductRow: View {
    let product: Product
    let isPurchased: Bool
    let onPurchase: () -> Void
    let onTap: () -> Void

    var body: some View {
        Button(action: self.onTap) {
            HStack {
                VStack(alignment: .leading) {
                    Text(self.product.displayName)
                        .font(.headline)
                        .foregroundColor(.primary)

                    Text(self.product.description)
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Text("Period: \(self.product.subscriptionPeriodFormatted() ?? "Could not format subscription period")")
                        .font(.caption)
                        .foregroundColor(.blue)

                    Text(self.product.displayPrice)
                        .font(.subheadline)
                        .fontWeight(.semibold)
                        .foregroundColor(.primary)
                }

                Spacer()

                VStack {
                    // Status indicator
                    HStack {
                        Circle()
                            .fill(self.isPurchased ? Color.green : Color.red)
                            .frame(width: 8, height: 8)

                        Text(self.isPurchased ? "Purchased" : "Not Purchased")
                            .font(.caption)
                            .foregroundColor(self.isPurchased ? .green : .red)
                    }

                    // Purchase button
                    if !self.isPurchased {
                        Button("Purchase") {
                            self.onPurchase()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                        .onTapGesture {
                            // Prevent navigation when tapping purchase button
                        }
                    } else {
                        Text("✓ Active")
                            .font(.caption)
                            .foregroundColor(.green)
                            .fontWeight(.semibold)
                    }
                }

                // Right arrow for navigation
                Image(systemName: "chevron.right")
                    .foregroundColor(.secondary)
                    .font(.caption)
            }
            .padding()
            .background(Color(.systemGray6))
            .cornerRadius(8)
        }
        .buttonStyle(PlainButtonStyle())
    }
}

private struct ProductDetailView: View {
    let product: Product
    let store: StoreKitModel
    @State private var detailedInfo: [String: String]?
    @State private var isLoading = true
    @State private var copiedField: String?
    @State private var favoriteTitles: [String] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if self.isLoading {
                HStack {
                    ProgressView()
                    Text("Loading product details...")
                        .foregroundColor(.secondary)
                }
                .padding()
            } else if let detailedInfo = detailedInfo {
                List {
                    // Favorites section
                    if !self.favoriteTitles.isEmpty {
                        Section("Favorites") {
                            ForEach(self.favoriteTitles, id: \.self) { title in
                                if let value = detailedInfo[title] {
                                    self.copyableInfoRow(
                                        title: title,
                                        value: value,
                                        copiedField: self.copiedField
                                    )
                                }
                            }
                        }
                    }

                    // All details section
                    Section("All Details") {
                        ForEach(Array(detailedInfo.keys.filter { !self.favoriteTitles.contains($0) }), id: \.self) { key in
                            if let value = detailedInfo[key] {
                                self.copyableInfoRow(
                                    title: key,
                                    value: value,
                                    copiedField: self.copiedField
                                )
                            }
                        }
                    }
                }
            } else {
                Text("Failed to load product details")
                    .foregroundColor(.red)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding()
            }
        }
        .navigationTitle(self.product.displayName)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await self.loadDetailedInfo()
        }
        .onAppear {
            self.loadFavoriteTitles()
        }
    }

    @ViewBuilder
    private func copyableInfoRow(title: String, value: String, copiedField: String?) -> some View {
        CopyableInfoRow(
            title: title,
            value: value,
            isCopied: copiedField == title,
            onCopy: {
                UIPasteboard.general.string = value
                self.copiedField = title
                // Reset copied state after 2 seconds
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                    if self.copiedField == title {
                        self.copiedField = nil
                    }
                }
            },
            onFavoriteToggle: {
                self.toggleFavorite(title)
            }
        )
    }

    private func loadDetailedInfo() async {
        self.isLoading = true
        // Find the corresponding ObscuraProduct enum case
        if let obscuraProduct = StoreKitModel.ObscuraProduct.allCases.first(
            where: { $0.rawValue == product.id
            })
        {
            self.detailedInfo = await self.store.getDetailedProductInformation(obscuraProduct: obscuraProduct)
        }
        self.isLoading = false
    }

    private func loadFavoriteTitles() {
        if let data = UserDefaults.standard.data(forKey: "favoriteProductTitles"),
           let titles = try? JSONDecoder().decode([String].self, from: data)
        {
            self.favoriteTitles = titles
        }
    }

    private func saveFavoriteTitles() {
        if let data = try? JSONEncoder().encode(favoriteTitles) {
            UserDefaults.standard.set(data, forKey: "favoriteProductTitles")
        }
    }

    private func toggleFavorite(_ title: String) {
        if self.favoriteTitles.contains(title) {
            self.favoriteTitles.removeAll { $0 == title }
        } else {
            self.favoriteTitles.append(title)
        }
        self.saveFavoriteTitles()
    }
}

// Warning! LLM generated. May not display best practice
private struct CopyableInfoRow: View {
    let title: String
    let value: String
    let isCopied: Bool
    let onCopy: () -> Void
    let onFavoriteToggle: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Text(self.title)
                .font(.caption2)
                .bold()
                .foregroundColor(.primary)
                .frame(width: 90, alignment: .leading)

            Text(self.value)
                .font(.caption2)
                .foregroundColor(.primary)
                .multilineTextAlignment(.leading)
                .layoutPriority(1)

            Spacer()

            Button(action: self.onCopy) {
                Image(systemName: self.isCopied ? "checkmark.circle.fill" : "doc.on.doc")
                    .foregroundColor(self.isCopied ? .green : .blue)
                    .frame(width: 5, height: 5)
            }
            .buttonStyle(PlainButtonStyle())
        }
        .contentShape(Rectangle())
        .onLongPressGesture {
            self.onFavoriteToggle()
        }
    }
}

extension StoreKitModel {
    @MainActor func getDetailedProductInformation(obscuraProduct: ObscuraProduct) async -> [String: String]? {
        guard let storekitProduct = availableStoreKitProductObject(
            obscuraProduct
        ) else {
            return nil
        }

        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "MM/dd/yyyy h:mma"

        var info: [String: String] = [
            "id": storekitProduct.id,
            "displayName": storekitProduct.displayName,
            "price": storekitProduct.price.formatted(),
            "description": storekitProduct.description,
            "type": storekitProduct.type.localizedDescription,
            "subscription": storekitProduct.subscriptionPeriodFormatted() ?? "Nil",
        ]

        // Add subscription-specific information
        if let subscription = storekitProduct.subscription {
            info["subscriptionGroupID"] = subscription.subscriptionGroupID
            info["subscriptionPeriod"] = "\(subscription.subscriptionPeriod.value) \(subscription.subscriptionPeriod.unit.localizedDescription)"
            if let introductoryOffer = subscription.introductoryOffer {
                info["introductoryOffer"] = "\(introductoryOffer.price.formatted()) for \(introductoryOffer.period.value) \(introductoryOffer.period.unit.localizedDescription)"
            }
            info["promotionalOffers"] = "\(subscription.promotionalOffers.count) available"
        }

        // Check if this product is currently purchased
        info["Is Currently Purchased"] = await self.isPurchased(product: storekitProduct) ? "Yes" : "No"

        if let latestTransaction = await storekitProduct.latestTransaction {
            info["latestTransaction"] = "Some!"
            switch latestTransaction {
            case .unverified:
                info["latestTransaction → verification"] = "Unverified"
            case .verified(let transaction):
                info["latestTransaction → purchaseDate"] = dateFormatter.string(from: transaction.purchaseDate)
                info["latestTransaction → originalPurchaseDate"] = dateFormatter.string(from: transaction.originalPurchaseDate)
                info["latestTransaction → transactionID"] = String(transaction.id)

                if let revocationDate = transaction.revocationDate {
                    info["latestTransaction → verified"] = "Verified but revoked on \(dateFormatter.string(from: revocationDate)) 😞"
                } else if let expirationDate = transaction.expirationDate {
                    info["latestTransaction → expirationDate"] = dateFormatter.string(from: expirationDate)
                    if expirationDate < Date() {
                        info["latestTransaction → verified"] = "Verified but expired \(dateFormatter.string(from: expirationDate)) 😞"
                    } else {
                        info["latestTransaction → verified"] = "Verified and Valid! ✅"
                    }
                } else if transaction.isUpgraded {
                    info["latestTransaction → verified"] = "Verified but upgraded 😕"
                } else {
                    info["latestTransaction → verified"] = "Verified and Valid! ✅"
                }

                // Add ownership type information
                info["latestTransaction → ownershipType"] = transaction.ownershipType.localizedDescription
            }

        } else {
            info["latestTransaction"] = "none"
        }
        return info
    }
}

#Preview {
    StoreDebugUI(storeKitModel: StoreKitModel(manager: nil), accountId: "nothing")
}
