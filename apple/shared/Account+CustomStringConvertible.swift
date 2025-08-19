import Foundation

private func descriptionOrNilString(_ object: CustomStringConvertible?) -> String {
    if let object {
        return "\(object)"
    } else {
        return "(nil)"
    }
}

private func shortRelativeTimeSubscription(_ date: Date) -> String {
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = RelativeDateTimeFormatter.UnitsStyle.abbreviated
    return formatter.localizedString(for: date, relativeTo: Date())
}

extension AccountInfo: CustomStringConvertible {
    var description: String {
        let str = "{AccountInfo -- id \(id), active \(active), topUp: \(descriptionOrNilString(topUp)), stripSubscription: \(descriptionOrNilString(stripeSubscription)), appleSubscription: \(descriptionOrNilString(appleSubscription))}"
        return str
    }
}

extension TopUpInfo: CustomStringConvertible {
    var description: String {
        "{TopUpInfo -- creditExpiresAt: \(shortRelativeTimeSubscription(self.creditExpiresAtDate))}"
    }
}

extension StripeSubscriptionInfo: CustomStringConvertible {
    var description: String {
        "{StripeSubscriptionInfo -- status: \(self.status.rawValue), currentPeriodStart: \(shortRelativeTimeSubscription(self.currentPeriodStartDate)), currentPeriodEnd: \(shortRelativeTimeSubscription(self.currentPeriodEndDate))"
    }
}

extension AppleSubscriptionInfo: CustomStringConvertible {
    var description: String {
        "{StripeSubscriptionInfo -- status: \(subscriptionStatus.description), autoRenewalStatus: \(autoRenewalStatus), renewalTime: \(shortRelativeTimeSubscription(self.renewalDate))}"
    }
}
