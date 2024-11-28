import Foundation

let utcDateFormat: DateFormatter = {
    var f = DateFormatter()
    f.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
    f.timeZone = TimeZone.gmt
    return f
}()
