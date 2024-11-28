// https://stackoverflow.com/a/74896180/7732434
func leftPad(_ str: String, toLength: Int, withPad character: Character) -> String {
    if str.count < toLength {
        return String(repeating: character, count: toLength - str.count) + str
    } else {
        return str
    }
}
