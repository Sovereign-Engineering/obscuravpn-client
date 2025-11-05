import NetworkExtension
import os
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "PurchaseOptionsView")

struct LabelledDivider: View {
    let label: String
    let horizontalPadding: CGFloat
    let color: Color

    init(label: String, horizontalPadding: CGFloat = 20, color: Color = .gray) {
        self.label = label
        self.horizontalPadding = horizontalPadding
        self.color = color
    }

    var body: some View {
        HStack {
            self.line
            Text(self.label).foregroundColor(self.color)
            self.line
        }
    }

    var line: some View {
        VStack { Divider().background(self.color) }.padding(self.horizontalPadding)
    }
}

struct PurchaseOptionsView: View {
    let openUrl: (URL) -> Void
    @ObservedObject var viewModel: SubscriptionManageViewModel
    @ObservedObject var storeKitModel: StoreKitModel

    @State private var manageSubscriptionsPopover: Bool = false
    @State private var restorePurchasesInProgress: Bool = false
    @State private var isPromoCodeSheetPresented: Bool = false
    @State private var promoCodeAccountAssociationError: Bool = false
    @State private var redeemCodeInProgress: Bool = false

    @State private var restoreLinkFlash: Bool = false
    @State private var redeemLinkFlash: Bool = false

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

    var body: some View {
        if let accountInfo = viewModel.accountInfo, let product = self.monthlySubscriptionProduct {
            if !accountInfo.active {
                VStack(alignment: .center, spacing: 18) {
                    VStack(alignment: .center, spacing: 14) {
                        self.inAppSubscriptionManageButton(product: product)

                        // Beware!!! redeemCodeButton are given 10pts of padding for their hit target to account for small text
                        // Make sure they are not spaced any closer than that
                        self.redeemCodeButton
                            .font(.footnote)
                        self.restorePurchasesButton
                            .font(.footnote)
                    }
                    // Account for padding coming out of nested v stack
                    .padding(-14)

                    // External payments are currently only straightforward in the US
                    if self.storeKitModel.storefront?.countryCode == "USA" {
                        LabelledDivider(label: "or")
                            .padding(.horizontal, 30)
                        self.externalPaymentManageButton(accountInfo: accountInfo)
                    }
                }
            } else if accountInfo.hasActiveAppleSubscription {
                VStack(alignment: .center, spacing: 14) {
                    self.inAppSubscriptionManageButton(product: product)
                    self.redeemCodeButton
                        .font(.footnote)
                }
            }
            if accountInfo.activeNotApple {
                self.externalPaymentManageButton(accountInfo: accountInfo)
            }
        }
    }

    func inAppSubscriptionManageButton(product: Product) -> some View {
        return VStack {
            VStack(alignment: .leading) {
                Text(product.displayName)
                    .font(.headline)
                    .foregroundColor(.primary)

                Text(product.description)
                    .font(.caption)
                    .foregroundColor(.secondary)

                product.subscriptionPeriodFormatted().map { period in
                    Text(period).font(.caption)
                }

                Text(product.displayPrice)
                    .font(.subheadline)
                    .fontWeight(.semibold)
                    .foregroundColor(.primary)
            }
            .padding()

            Button {
                if self.storeKitModel.hasActiveMonthlySubscription {
                    self.manageSubscriptionsPopover = true
                } else {
                    Task {
                        try await self.viewModel.purchaseSubscription()
                    }
                }
            } label: {
                if self.storeKitModel.hasActiveMonthlySubscription {
                    Text("Manage Subscription")
                        .frame(maxWidth: .infinity)
                } else {
                    Text("Subscribe In-app")
                        .frame(maxWidth: .infinity)
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, self.storeKitModel.hasActiveMonthlySubscription ? 35 : 45)
            .bold()
            .tint(Color.obscuraOrange)
            .manageSubscriptionsSheet(
                isPresented: self.$manageSubscriptionsPopover
            )
        }
    }

    var restorePurchasesButton: some View {
        HStack {
            Text("Restore Purchases")
                .underline()
                .foregroundColor((self.restorePurchasesInProgress || self.restoreLinkFlash) ? .gray : .blue)
        }
        .padding(10)
        .onTapGesture {
            Task { @MainActor in
                self.restorePurchasesInProgress = true
                await self.storeKitModel.restorePurchases()
                self.restorePurchasesInProgress = false
            }

            // Animation for pressing. We cant use button because we want to expand hit target
            withAnimation(.easeInOut(duration: 0.15)) { self.restoreLinkFlash = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
                withAnimation(.easeInOut(duration: 0.15)) { self.restoreLinkFlash = false }
            }
        }
        .contentShape(Rectangle())
        .accessibilityAddTraits(.isButton)
        .padding(-10)
        .disabled(self.restorePurchasesInProgress)
        .contentShape(Rectangle())
        .manageSubscriptionsSheet(
            isPresented: self.$manageSubscriptionsPopover
        )
    }

    @ViewBuilder
    func externalPaymentManageButton(accountInfo: AccountInfo) -> some View {
        Button {
            self.openUrl(
                URL(
                    string: "https://obscura.net/pay/#account_id=\(accountInfo.id)&external"
                )!
            )
        } label: {
            HStack {
                Text(
                    accountInfo.active ? "Manage Payment on obscura.net" : "Pay on obscura.net"
                )

                Image(systemName: "arrow.up.right")
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
        .padding(.horizontal, accountInfo.active ? 10 : 30)
        .bold()
        .tint(Color.obscuraOrange)
        .conditionallyDisabled(
            when: accountInfo.hasActiveAppleSubscription || self.storeKitModel.hasActiveMonthlySubscription,
            explanation: "You're already subscribed through the App Store! You can't pay externally until your subscription expires."
        )
    }

    var redeemCodeButton: some View {
        HStack {
            Text("Have a promo code?")
            Text("Redeem Code")
                .underline()
                .foregroundColor((self.redeemLinkFlash || self.redeemCodeInProgress) ? .gray : .blue)
                .padding(10)
                .onTapGesture(perform: {
                    self.redeemCodeInProgress = true

                    Task { @MainActor in
                        do {
                            try await self.storeKitModel.associateAccount()
                            self.isPromoCodeSheetPresented = true
                        } catch {
                            logger.error("Failed to Associate Apple account: \(error, privacy: .public)")
                            self.promoCodeAccountAssociationError = true
                        }
                    }

                    // Animation for pressing. We cant use button because we want to expand hit target
                    withAnimation(.easeInOut(duration: 0.15)) { self.redeemLinkFlash = true }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
                        withAnimation(.easeInOut(duration: 0.15)) { self.redeemLinkFlash = false }
                    }
                })
                .contentShape(Rectangle())
                .accessibilityAddTraits(.isButton)
                .padding(-10)
        }
        .alert("Failed to redeem promo code.", isPresented: self.$promoCodeAccountAssociationError) {
            Button("OK") {}
        }
        .offerCodeRedemption(isPresented: self.$isPromoCodeSheetPresented) { result in
            Task { @MainActor in
                self.redeemCodeInProgress = false
            }
            switch result {
            case .success:
                Task {
                    await self.viewModel.onOfferCodeRedemption()
                }
                logger.info("Promo code redemption flow completed successfully. (errors only show up if a valid code fails to redeem. So invalid codes and not entering a code land you here)")
            case .failure(let error):
                logger.error("Promo code redemption failed: \(error)")
            }
        }
        .disabled(self.redeemCodeInProgress)
    }
}
