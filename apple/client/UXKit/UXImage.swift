/*
 Many UIKit and AppKit classes have fairly similar interfaces
 To that end you can get away with code like this. There are libraries out there
 With a more complete set but I did not wnat to add that dependency given we need such a
 small subset
 https://github.com/ZeeZide/UXKit
 */

import SwiftUI

#if os(macOS)
    import AppKit

    typealias UXImage = NSImage
#else
    import UIKit

    typealias UXImage = UIImage
#endif

extension Image {
    init(uxImage: UXImage) {
        #if os(macOS)
            self.init(nsImage: uxImage)
        #else
            self.init(uiImage: uxImage)
        #endif
    }
}
