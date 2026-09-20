import Foundation

struct AccountStatus: Codable, Equatable {
    var accountInfo: AccountInfo
    var lastUpdatedSec: UInt64

    enum CodingKeys: String, CodingKey {
        case accountInfo = "account_info"
        case lastUpdatedSec = "last_updated_sec"
    }

    var periodEnd: Int64? {
        return self.accountInfo.currentExpiry ?? self.accountInfo.autoRenews
    }

    /// returns the expected date when
    ///   1) the account's subscription renews
    ///   2) or the account will expire
    /// this is used to determined when the account info should be refreshed
    var periodEndDate: Date? {
        return self.periodEnd.map { Date(timeIntervalSince1970: TimeInterval($0)) }
    }

    // returns nil when:
    //  subscription is active and renewing
    // returns zero when:
    //  never topped up
    // returns a date in the past when:
    //  account is past its expiration date (or never funded)
    private var expirationDate: Date? {
        if self.accountInfo.autoRenews != nil {
            return nil
        }
        return Date(timeIntervalSince1970: TimeInterval(self.accountInfo.currentExpiry ?? 0))
    }

    func daysUntilExpiry() -> UInt64? {
        if !self.accountInfo.active {
            return 0
        }
        if let end = self.expirationDate {
            let now = Date()
            return UInt64(max(Calendar.current.dateComponents([.day], from: now, to: end).day ?? 0, 0))
        }
        return nil
    }

    var isActive: Bool {
        self.accountInfo.active
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
    let active: Bool
    let autoRenews: Int64?
    let currentExpiry: Int64?

    enum CodingKeys: String, CodingKey {
        case active
        case autoRenews = "auto_renews"
        case currentExpiry = "current_expiry"
    }
}

extension AccountInfo: CustomStringConvertible {
    var description: String {
        let str = "{AccountInfo -- active \(active), autoRenews \(autoRenews, default: "(nil)"), currentExpiry \(currentExpiry, default: "(nil)")}"
        return str
    }
}

// https://github.com/Sovereign-Engineering/obscuravpn-api/blob/main/src/cmd/apple/associate_account.rs
struct AppleAssociateAccountOutput: Codable {}
