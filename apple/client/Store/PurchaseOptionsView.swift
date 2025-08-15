import NetworkExtension
import os
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "PurchaseOptionsView")

struct PurchaseOptionsView: View {
    let openUrl: (URL) -> Void
    @ObservedObject var viewModel: SubscriptionManageViewModel
    @ObservedObject var storeKitModel: StoreKitModel

    @State private var manageSubscriptionsPopover: Bool = false
    @State private var restorePurchasesInProgress: Bool = false
    @State private var isPromoCodeSheetPresented: Bool = false
    @State private var promoCodeAccountAssociationError: Bool = false

    init(
        openUrl: @escaping (URL) -> Void,
        viewModel: SubscriptionManageViewModel
    ) {
        self.openUrl = openUrl
        self.viewModel = viewModel
        self.storeKitModel = viewModel.storeKitModel
    }

    private var monthlySubscriptionProduct: Product? {
        return self.storeKitModel.product(for: .monthlySubscription)
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
            if let monthlySubscriptionProduct, let accountInfo = viewModel.accountInfo, !accountInfo.hasActiveAppleSubscription, !storeKitModel.hasActiveMonthlySubscription {
                ProductButton(
                    displayName: monthlySubscriptionProduct.displayName,
                    description: monthlySubscriptionProduct.description,
                    displayPrice: self.displayPrice ?? "",
                    purchaseClosure: {
                        try await self.viewModel.purchaseSubscription()
                    }
                )
                .conditionallyDisabled(
                    when: accountInfo.hasActiveExternalPaymentPlan,
                    explanation: accountInfo.hasStripeSubscription ? alreadyStripeSubscribed : alreadyToppedUp
                )

                self.restorePurchasesButton

                self.redeemCodeButton
            }

            if self.viewModel.accountInfo?.hasActiveAppleSubscription ?? false {
                self.manageSubscriptionButton
            }

            // External payments are currently only straightforward in the US
            if self.storeKitModel.storefront?.countryCode == "USA" {
                if let accountInfo = self.viewModel.accountInfo {
                    self.externalPaymentButton(accountInfo: accountInfo)
                }
            }
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

    @ViewBuilder
    func externalPaymentButton(accountInfo: AccountInfo) -> some View {
        Button {
            self.openUrl(
                URL(
                    string: "https://obscura.net/pay/#account_id=\(accountInfo.id)&external"
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
        .buttonStyle(HyperlinkButtonStyle())
        .conditionallyDisabled(
            when: accountInfo.hasActiveAppleSubscription || self.storeKitModel.hasActiveMonthlySubscription,
            explanation: "You're already subscribed through the App Store! You can't pay externally until your subscription expires."
        )
    }

    var redeemCodeButton: some View {
        Button {
            Task { @MainActor in
                do {
                    try await self.storeKitModel.associateAccount()
                    self.isPromoCodeSheetPresented = true
                } catch {
                    logger.error("Failed to Associate Apple account: \(error, privacy: .public)")
                    self.promoCodeAccountAssociationError = true
                }
            }
        } label: {
            Text("Redeem Code")
        }
        .alert("Failed to link Apple and Obscura account. Please try again later.", isPresented: self.$promoCodeAccountAssociationError) {
            Button("OK") {}
        }
        .offerCodeRedemption(isPresented: self.$isPromoCodeSheetPresented) { result in
            switch result {
            case .success:
                logger.info("Promo code redemption flow completed successfully. (errors only show up if a valid code fails to redeem. So invalid codes and not entering a code land you here)")
            case .failure(let error):
                logger.error("Promo code redemption failed: \(error)")
            }
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
