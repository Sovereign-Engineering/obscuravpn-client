import Foundation

extension AccountInfo {
    var hasTopUp: Bool {
        guard let topUp else { return false }

        return topUp.creditExpiresAtDate > .now
    }

    var hasStripeSubscription: Bool {
        guard let stripeSubscription else { return false }
        if self.hasRenewingStripeSubscription { return true }
        let expirationDate = Date(
            timeIntervalSince1970: TimeInterval(
                stripeSubscription.currentPeriodEnd
            )
        )
        return expirationDate > .now
    }

    var hasActiveExternalPaymentPlan: Bool {
        return (
            self.hasTopUp || self.hasStripeSubscription
        ) && !self.hasActiveAppleSubscription
    }

    var hasActiveAppleSubscription: Bool {
        guard let appleSubscription else {
            return false
        }
        return appleSubscription.subscriptionStatus == .active || appleSubscription.subscriptionStatus == .billingRetry || appleSubscription.subscriptionStatus == .gracePeriod
    }
}
