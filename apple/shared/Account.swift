struct AccountStatus: Codable, Equatable {
    var accountInfo: AccountInfo
    var daysTillExpiry: UInt64?
    var lastUpdatedSec: UInt64

    enum CodingKeys: String, CodingKey {
        case accountInfo = "account_info"
        case daysTillExpiry = "days_till_expiry"
        case lastUpdatedSec = "last_updated_sec"
    }

    func expiringSoon() -> Bool {
        if let daysTillExpiry = self.daysTillExpiry {
            return daysTillExpiry <= 10
        } else {
            return false
        }
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

struct AccountDaysTillExpiry {
    var days: Int64?
    func expiringSoon() -> Bool {
        return self.days != nil && self.days! <= 10
    }
}
