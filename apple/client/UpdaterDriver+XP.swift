import Foundation

enum UpdaterStatusType: String, Codable {
    case uninitiated
    case initiated
    case available
    case notFound
    case error
}

struct AppcastSummary: Codable {
    var date: String
    var description: String
    var version: String
    var minSystemVersionOk: Bool
}

struct UpdaterStatus: Codable, CustomStringConvertible {
    var description: String {
        return "UpdaterStatus(type: \(self.type), appcast: \(self.appcast as Optional), error: \(self.error as Optional)), errorCode: \(self.errorCode as Optional)"
    }

    var type: UpdaterStatusType = .uninitiated
    var appcast: AppcastSummary?
    var error: String?
    var errorCode: Int32?
}
