import SwiftUI

struct SubscriptionManageSheetViewPreviewCarousel: View {
    private struct SheetConfiguration {
        let title: String
        let accountInfo: AccountInfo
    }

    // MARK: - No-op URL handler

    private let noOpUrlHandler: (URL) -> Void = { _ in }

    private var configurations: [SheetConfiguration] {
        [
            self.noSubscriptionsConfig,
            self.topUpOnlyConfig,
            self.stripeSubscriptionOnlyConfig,
            self.appleSubscriptionOnlyConfig,
        ]
    }

    // MARK: - Configurations

    private var noSubscriptionsConfig: SheetConfiguration {
        SheetConfiguration(
            title: "No Stripe, No App Store",
            accountInfo: AccountInfo(
                id: "22222222222222222222",
                active: false,
                topUp: nil,
                stripeSubscription: nil,
                appleSubscription: nil,
                _autoRenews: nil,
                currentExpiry: nil
            )
        )
    }

    private var topUpOnlyConfig: SheetConfiguration {
        let futureDate = Int64(Date().addingTimeInterval(60 * 60 * 24 * 365).timeIntervalSince1970) // 1 year from now
        return SheetConfiguration(
            title: "Top Up Only",
            accountInfo: AccountInfo(
                id: "22222222222222222222",
                active: true,
                topUp: TopUpInfo(creditExpiresAt: futureDate),
                stripeSubscription: nil,
                appleSubscription: nil,
                _autoRenews: nil,
                currentExpiry: futureDate
            )
        )
    }

    private var stripeSubscriptionOnlyConfig: SheetConfiguration {
        let now = Int64(Date().timeIntervalSince1970)
        let futureDate = Int64(Date().addingTimeInterval(60 * 60 * 24 * 365).timeIntervalSince1970) // 1 year from now
        return SheetConfiguration(
            title: "Stripe Subscription Only",
            accountInfo: AccountInfo(
                id: "22222222222222222222",
                active: true,
                topUp: nil,
                stripeSubscription: StripeSubscriptionInfo(
                    status: .active,
                    currentPeriodStart: now,
                    currentPeriodEnd: futureDate,
                    cancelAtPeriodEnd: false
                ),
                appleSubscription: nil,
                _autoRenews: futureDate,
                currentExpiry: nil
            )
        )
    }

    private var appleSubscriptionOnlyConfig: SheetConfiguration {
        let futureDate = Int64(Date().addingTimeInterval(60 * 60 * 24 * 365).timeIntervalSince1970) // 1 year from now
        return SheetConfiguration(
            title: "Apple Subscription Only",
            accountInfo: AccountInfo(
                id: "22222222222222222222",
                active: true,
                topUp: nil,
                stripeSubscription: nil,
                appleSubscription: AppleSubscriptionInfo(
                    status: 1, // Active status
                    autoRenewalStatus: true,
                    renewalTime: futureDate
                ),
                _autoRenews: futureDate,
                currentExpiry: nil
            )
        )
    }

    var body: some View {
        TabView {
            ForEach(self.configurations.indices, id: \.self) { index in
                let config = self.configurations[index]
                VStack {
                    Text(config.title)
                        .font(.title)
                    SubscriptionManageSheetView(
                        viewModel: SubscriptionManageViewModel(
                            manager: nil,
                            accountInfo: config.accountInfo
                        ),
                        openUrl: self.noOpUrlHandler
                    )
                    .navigationTitle(config.title)
                }
            }
        }
        .tabViewStyle(.page)
    }
}
