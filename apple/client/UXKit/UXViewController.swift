/*
 Many UIKit and AppKit classes have fairly similar interfaces
 To that end you can get away with code like this. There are libraries out there
 With a more complete set but I did not wnat to add that dependency given we need such a
 small subset
 https://github.com/ZeeZide/UXKit
 */

#if os(macOS)
    import AppKit

    typealias UXViewController = NSViewController
#else
    import UIKit

    typealias UXViewController = UIViewController
#endif
