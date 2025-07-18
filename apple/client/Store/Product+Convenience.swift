import StoreKit

extension Product {
    func subscriptionPeriodFormatted() -> String? {
        guard let subscription else { return nil }
        return subscription.subscriptionPeriod.formatted(self.subscriptionPeriodFormatStyle)
    }
}
