// https://stackoverflow.com/a/74896180/7732434
func leftPad(_ str: String, toLength: Int, withPad character: Character) -> String {
    if str.count < toLength {
        return String(repeating: character, count: toLength - str.count) + str
    } else {
        return str
    }
}

// https://forums.swift.org/t/getting-the-name-of-a-swift-enum-value/35654/18
@_silgen_name("swift_EnumCaseName")
func _getEnumCaseName<T>(_ value: T) -> UnsafePointer<CChar>?

func getEnumCaseName<T>(for value: T) -> String? {
    if let stringPtr = _getEnumCaseName(value) {
        return String(validatingUTF8: stringPtr)
    }
    return nil
}
