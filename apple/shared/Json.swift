import Foundation
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "json")

extension Encodable {
    func json(function: String = #function, file: String = #fileID, line: Int = #line) throws -> String {
        do {
            let json = try JSONEncoder().encode(self)
            return String(data: json, encoding: .utf8)!
        } catch let err {
            logger.error("JSON encoding failed (\(file, privacy: .public):\(function, privacy: .public):\(line, privacy: .public)): \(err, privacy: .private)")
            throw errorCodeOther
        }
    }
}

extension Decodable {
    init(json: Data, function: String = #function, file: String = #fileID, line: Int = #line) throws {
        do {
            self = try JSONDecoder().decode(Self.self, from: json)
        } catch let err {
            logger.error("JSON decoding failed (\(file, privacy: .public):\(function, privacy: .public):\(line, privacy: .public)): \(err, privacy: .private)")
            throw errorCodeOther
        }
    }

    init(json: String, function: String = #function, file: String = #fileID, line: Int = #line) throws {
        try self.init(json: json.data(using: .utf8)!, function: function, file: file, line: line)
    }
}

/// Mutates the values in a dictionary so that they are able to be JSON encoded.
///
/// For it just converts binary data to Base 64.
func prepareForJson(_ value: inout Any) {
    switch value {
    case var array as [Any]:
        for (i, v) in array.enumerated() {
            var updated = v
            prepareForJson(&updated)
            array[i] = updated
        }
        value = array
    case var dict as [String: Any]:
        for (k, v) in dict {
            var updated = v
            prepareForJson(&updated)
            dict[k] = updated
        }
        value = dict
    case let data as Data:
        value = data.base64EncodedString()
    default:
        if !JSONSerialization.isValidJSONObject([value]) {
            value = debugFormat(value)
        }
    }
}
