import Foundation

struct AccountStatus: Codable, Equatable {
    var accountInfo: AccountInfo
    var lastUpdatedSec: UInt64

    enum CodingKeys: String, CodingKey {
        case accountInfo = "account_info"
        case lastUpdatedSec = "last_updated_sec"
    }

    func expirationDate() -> Date? {
        if let subscription = accountInfo.stripeSubscription {
            if !subscription.cancelAtPeriodEnd {
                return nil
            }
        }
        let top_up_end = self.accountInfo.topUp?.creditExpiresAt ?? 0
        let subscription_end = self.accountInfo.stripeSubscription?.currentPeriodEnd ?? 0
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
    let stripeSubscription: StripeSubscriptionInfo?
    let appleSubscription: AppleSubscriptionInfo?

    enum CodingKeys: String, CodingKey {
        case topUp = "top_up"
        case id
        case active
        case stripeSubscription = "subscription"
        case appleSubscription = "apple_subscription"
    }
}

struct TopUpInfo: Codable {
    let creditExpiresAt: Int64

    enum CodingKeys: String, CodingKey {
        case creditExpiresAt = "credit_expires_at"
    }
}

extension TopUpInfo {
    var expiryDate: Date {
        return Date(timeIntervalSince1970: TimeInterval(self.creditExpiresAt))
    }
}

struct StripeSubscriptionInfo: Codable {
    let status: StripeSubscriptionStatus
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

enum StripeSubscriptionStatus: String, Codable {
    case active
    case canceled
    case incomplete
    case incompleteExpired = "incomplete_expired"
    case pastDue = "past_due"
    case paused
    case trialing
    case unpaid
}

struct AppleSubscriptionInfo: Codable {
    // https://developer.apple.com/documentation/appstoreserverapi/status
    let status: Int32
    let autoRenewalStatus: Bool
    let renewalDate: Int64

    enum CodingKeys: String, CodingKey {
        case status
        case autoRenewalStatus = "auto_renew_status"
        case renewalDate = "renewal_date"
    }

    enum Status: Int32 {
        case active = 1
        case expired = 2
        case billingRetry = 3
        case gracePeriod = 4
        case revoked = 5

        var description: String {
            switch self {
            case .active:
                "Active"
            case .expired:
                "Expired"
            case .billingRetry:
                "In Billing Retry Period"
            case .gracePeriod:
                "In Billing Grace Period"
            case .revoked:
                "Revoked"
            }
        }
    }

    var subscriptionStatus: Status {
        Status(rawValue: self.status) ?? .expired
    }
}

// Output types for Apple subscription management
struct AppleCreateAppAccountTokenOutput: Codable {
    let appAccountToken: UUID

    enum CodingKeys: String, CodingKey {
        case appAccountToken = "app_account_token"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let tokenString = try container.decode(String.self, forKey: .appAccountToken)
        guard let uuid = UUID(uuidString: tokenString) else {
            throw DecodingError.dataCorruptedError(forKey: .appAccountToken, in: container, debugDescription: "Could not parse UUID string")
        }
        self.appAccountToken = uuid
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(self.appAccountToken.uuidString, forKey: .appAccountToken)
    }
}

