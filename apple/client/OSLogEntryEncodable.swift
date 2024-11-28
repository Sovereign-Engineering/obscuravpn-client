import OSLog

enum OSLogEntryCodingKeys: String, CodingKey {
    case activityIdentifier
    case category
    case components
    case eventMessage
    case eventType
    case formatString
    case messageType
    case parentActivityIdentifier
    case processID
    case processImagePath
    case senderImagePath
    case signpostID
    case signpostName
    case signpostType
    case subsystem
    case threadID
    case timestamp
}

extension OSLogEntry: Encodable {
    // Format a log matching `log show --style=ndjson`
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: OSLogEntryCodingKeys.self)

        try container.encode(self.date, forKey: .timestamp)
        try container.encode(self.composedMessage, forKey: .eventMessage)

        let type = switch self {
        case is OSLogEntryActivity: "activityCreateEvent"
        case is OSLogEntryBoundary: "boundary" // TODO: What is the official name?
        case is OSLogEntryLog: "logEvent"
        case is OSLogEntrySignpost: "signpostEvent"
        default: "unknown"
        }

        try container.encode(type, forKey: .eventType)

        switch self {
        case let entry as OSLogEntryActivity:
            try container.encode(entry.parentActivityIdentifier, forKey: .parentActivityIdentifier)
        case is OSLogEntryBoundary:
            break // No extra data.
        case let entry as OSLogEntryLog:
            let level = switch entry.level {
            case .undefined: nil as String?
            case .debug: "Debug"
            case .info: "Info"
            case .notice: "Default"
            case .error: "Error"
            case .fault: "Fault"
            default: "unknown"
            }
            try container.encode(level, forKey: .messageType)
        case let entry as OSLogEntrySignpost:
            try container.encode(entry.signpostIdentifier, forKey: .signpostID)
            try container.encode(entry.signpostName, forKey: .signpostName)

            let type = switch entry.signpostType {
            case .undefined: nil as String?
            case .intervalBegin: "begin"
            case .intervalEnd: "end"
            case .event: "event"
            default: "unknown"
            }
            try container.encode(type, forKey: .signpostType)
        default:
            break
        }

        if let entry = self as? OSLogEntryFromProcess {
            try container.encode(entry.activityIdentifier, forKey: .activityIdentifier)
            try container.encode(entry.process, forKey: .processImagePath) // We only get the filename, whereas the log command gets the full path.
            try container.encode(entry.processIdentifier, forKey: .processID)
            try container.encode(entry.sender, forKey: .senderImagePath) // We only get the filename, whereas the log command gets the full path.
            try container.encode(entry.threadIdentifier, forKey: .threadID)
        }
        if let entry = self as? OSLogEntryWithPayload {
            try container.encode(entry.category, forKey: .category)
            try container.encode(entry.formatString, forKey: .formatString)
            try container.encode(entry.subsystem, forKey: .subsystem)

            // The log command doesn't break these out, but it makes analysis easier.
            var components = container.nestedUnkeyedContainer(forKey: .components)
            for component in entry.components {
                switch component.argument {
                case .data(let data):
                    try components.encode(data)
                case .double(let num):
                    try components.encode(num)
                case .signed(let num):
                    try components.encode(num)
                case .string(let str):
                    try components.encode(str)
                case .undefined:
                    try components.encode(nil as String?)
                case .unsigned(let num):
                    try components.encode(num)
                @unknown default:
                    try components.encode("unknown-type")
                }
            }
        }
    }
}
