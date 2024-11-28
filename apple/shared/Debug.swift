func debugFormat(_ v: Any?) -> String {
    guard let v = v else { return "nil" }

    var r = ""
    debugPrint(v, terminator: "", to: &r)
    return r
}
