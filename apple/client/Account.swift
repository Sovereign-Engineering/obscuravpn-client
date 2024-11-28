struct AccountInfo: Decodable, Encodable {
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

struct TopUpInfo: Decodable, Encodable {
    let creditExpiresAt: Int64

    enum CodingKeys: String, CodingKey {
        case creditExpiresAt = "credit_expires_at"
    }
}

struct SubscriptionInfo: Decodable, Encodable {
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

enum SubscriptionStatus: String, Decodable, Encodable {
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
