import Foundation

struct AccountStatus: Codable, Equatable {
    var accountInfo: AccountInfo
    var lastUpdatedSec: UInt64

    enum CodingKeys: String, CodingKey {
        case accountInfo = "account_info"
        case lastUpdatedSec = "last_updated_sec"
    }

    func expirationDate() -> Date? {
        if let subscription = accountInfo.subscription {
            if !subscription.cancelAtPeriodEnd {
                return nil
            }
        }
        let top_up_end = self.accountInfo.topUp?.creditExpiresAt ?? 0
        let subscription_end = self.accountInfo.subscription?.currentPeriodEnd ?? 0
        let end = max(top_up_end, subscription_end, 0)
        return Date(timeIntervalSince1970: TimeInterval(end))
    }

    func daysUntilExpiry() -> UInt64? {
        if !self.accountInfo.active {
            return 0
        }
        if let end = self.expirationDate() {
            let now = Date()
            return UInt64(max(Calendar.current.dateComponents([.day], from: now, to: end).day ?? 0, 0))
        }
        return nil
    }

    func isActive() -> Bool {
        if let timestamp = self.expirationDate() {
            return timestamp > Date()
        }
        return self.accountInfo.active
    }

    func expiringSoon() -> Bool {
        if let daysTillExpiry = daysUntilExpiry() {
            return daysTillExpiry <= 10
        }
        return false
    }

    static func == (left: AccountStatus, right: AccountStatus) -> Bool {
        return left.lastUpdatedSec == right.lastUpdatedSec
    }
}

struct AccountInfo: Codable {
    let id: String
    let active: Bool
    let topUp: TopUpInfo?
    let subscription: SubscriptionInfo?

    enum CodingKeys: String, CodingKey {
        case topUp = "top_up"
        case id
        case active
        case subscription
    }
}

struct TopUpInfo: Codable {
    let creditExpiresAt: Int64

    enum CodingKeys: String, CodingKey {
        case creditExpiresAt = "credit_expires_at"
    }
}

struct SubscriptionInfo: Codable {
    let status: SubscriptionStatus
    let currentPeriodStart: Int64
    let currentPeriodEnd: Int64
    let cancelAtPeriodEnd: Bool

    enum CodingKeys: String, CodingKey {
        case currentPeriodStart = "current_period_start"
        case currentPeriodEnd = "current_period_end"
        case cancelAtPeriodEnd = "cancel_at_period_end"
        case status
    }
}

enum SubscriptionStatus: String, Codable {
    case active
    case canceled
    case incomplete
    case incompleteExpired = "incomplete_expired"
    case pastDue = "past_due"
    case paused
    case trialing
    case unpaid
}
