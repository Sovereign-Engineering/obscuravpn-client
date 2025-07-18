import NetworkExtension
import os
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "PurchaseOptionsView")

struct PurchaseOptionsView: View {
    let accountInfo: AccountInfo
    let openUrl: (URL) -> Void
    @ObservedObject var storeKitModel: StoreKitModel
    let manager: NETunnelProviderManager?

    @State private var manageSubscriptionsPopover: Bool = false
    @State private var restorePurchasesInProgress: Bool = false

    private var monthlySubscriptionProduct: Product? {
        self.storeKitModel.product(for: .monthlySubscription)
    }

    private var displayPrice: String? {
        if let monthlySubscriptionProduct, let subscriptionPeriodFormatted = monthlySubscriptionProduct.subscriptionPeriodFormatted() {
            return "\(monthlySubscriptionProduct.displayPrice)/\n\(subscriptionPeriodFormatted)"
        }
        return nil
    }

    var body: some View {
        let alreadyStripeSubscribed = "You're already subscribed through Stripe! You can't subscribe through the App Store until your subscription expires."
        let alreadyToppedUp = "You're already topped-up! You can't subscribe through the App Store until your top-up expires."
        VStack(alignment: .leading, spacing: 24) {
            if let monthlySubscriptionProduct, !accountInfo.hasActiveAppleSubscription {
                ProductButton(
                    displayName: monthlySubscriptionProduct.displayName,
                    description: monthlySubscriptionProduct.description,
                    displayPrice: self.displayPrice ?? "",
                    purchaseClosure: {
                        try await self.purchaseSubscription()
                    }
                )
                .conditionallyDisabled(
                    when: self.accountInfo.hasActiveExternalPaymentPlan,
                    explanation: self.accountInfo.hasStripeSubscription ? alreadyStripeSubscribed : alreadyToppedUp
                )
            }

            if self.accountInfo.hasActiveAppleSubscription {
                self.manageSubscriptionButton
            }

            self.restorePurchasesButton

            self.externalPaymentButton
        }
    }

    var manageSubscriptionButton: some View {
        Button {
            self.manageSubscriptionsPopover = true
        } label: {
            Text("Manage Subscription")
        }
        .manageSubscriptionsSheet(
            isPresented: self.$manageSubscriptionsPopover
        )
    }

    var restorePurchasesButton: some View {
        ZStack {
            Button {
                Task { @MainActor in
                    self.restorePurchasesInProgress = true
                    await self.storeKitModel.restorePurchases()
                    self.restorePurchasesInProgress = false
                }
            } label: {
                Text("Restore Purchases")
            }
            .manageSubscriptionsSheet(
                isPresented: self.$manageSubscriptionsPopover
            )
            .disabled(self.restorePurchasesInProgress)

            if self.restorePurchasesInProgress {
                ProgressView()
            }
        }
    }

    var externalPaymentButton: some View {
        Button {
            self.openUrl(
                URL(
                    string: "https://obscura.net/pay/#account_id=\(self.accountInfo.id)&external"
                )!
            )
        } label: {
            HStack {
                Text("Pay on Obscura.net")
                    .font(.body)

                Image(systemName: "arrow.up.right")
                    .font(.caption)
            }
        }
        .foregroundColor(.blue)
        .conditionallyDisabled(
            when: self.accountInfo.hasActiveAppleSubscription,
            explanation: "You're already subscribed through the App Store! You can't pay externally until your subscription expires."
        )
    }

    func purchaseSubscription() async throws {
        guard let manager else {
            logger.error("Cannot purchaseSubscription without a manager")
            return
        }
        let persistedTokenMappings = PersistedAppAccountTokenMappings()

        // Get appAccountToken
        let appAccountToken: UUID
        if let existingToken = persistedTokenMappings.getAccountToken(
            for: accountInfo
                .id)
        {
            appAccountToken = existingToken
        } else {
            do {
                appAccountToken = try await neApiAppleCreateAppAccountToken(
                    manager
                ).appAccountToken
                persistedTokenMappings
                    .setAccountToken(
                        accountId: self.accountInfo.id,
                        appAccountToken: appAccountToken
                    )
            } catch {
                logger.error("Failed to get app account token: \(error)")
                throw error
            }
        }

        // Purchase
        do {
            try await self.storeKitModel.purchase(obscuraProduct: .monthlySubscription, appAccountToken: appAccountToken)
        } catch {
            logger.error("Purchase failed: \(error)")
            throw error
        }
    }
}

private struct ProductButton: View {
    let displayName: String
    let description: String
    let displayPrice: String
    let color: Color = .init("ObscuraOrange")
    let purchaseClosure: () async throws -> Void
    @State private var isLoading = false

    // Workaround :( For some reason SwiftUI animation system cannot keep up with Configuration.isPressed in this view
    // Normally you should use ButtonStyle + Configuration.isPressed
    @State private var isPressed: Bool = false

    var body: some View {
        Button {
            // HACK!
            self.isPressed = true
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(0.15))
                self.isPressed = false
            }

            Task {
                self.isLoading = true
                do {
                    try await self.purchaseClosure()
                } catch {
                    print("Purchase failed: \(error)")
                }
                self.isLoading = false
            }
        } label: {
            HStack {
                HStack(spacing: 12) {
                    Image(systemName: "repeat")
                        .font(.title)

                    VStack(alignment: .leading, spacing: 4) {
                        Text(self.displayName)
                            .font(.headline)
                            .lineLimit(1)
                            .fixedSize()
                        Text(self.description)
                            .multilineTextAlignment(.leading)
                            .font(.subheadline)
                            .lineLimit(2)
                            .foregroundColor(.secondary)
                    }
                }

                Spacer()

                Text(self.displayPrice)
                    .font(.subheadline)
                    .lineLimit(2)
                    .fixedSize()
            }
        }
        .foregroundColor(self.color)
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color(.systemGray6))
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(self.color, lineWidth: 1)
                )
        )
        .scaleEffect(self.isPressed ? 0.98 : 1.0)
        .animation(.easeOut(duration: 0.15), value: self.isPressed)
        .saturation(self.isLoading ? 0.3 : 1)
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
