import Foundation

public class DebugBundleStatus: Encodable {
    var inProgressCounter: Int = 0
    var inProgress: Bool {
        return self.inProgressCounter > 0
    }

    var latestPath: String?

    func start() {
        self.inProgressCounter += 1
    }

    func finish() {
        self.inProgressCounter -= 1
    }

    func setPath(_ path: String) {
        self.latestPath = path
    }

    func markError() {
        self.latestPath = nil
    }

    enum CodingKeys: String, CodingKey {
        case inProgressCounter
        case inProgress
        case latestPath
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(self.inProgressCounter, forKey: .inProgressCounter)
        try container.encode(self.inProgress, forKey: .inProgress)
        try container.encode(self.latestPath, forKey: .latestPath)
    }
}
