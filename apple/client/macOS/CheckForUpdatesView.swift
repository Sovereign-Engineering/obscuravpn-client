import Sparkle
import SwiftUI

/**
 This is the view for the Check for Updates menu item

 Note this intermediate view is necessary for the disabled state on the menu item to work properly before Monterey.
 See https://stackoverflow.com/questions/68553092/menu-not-updating-swiftui-bug for more info.
 **/
struct CheckForUpdatesView: View {
    @State var canCheckForUpdates: Bool = false

    private let updater: SparkleUpdater

    init(updater: SparkleUpdater) {
        self.updater = updater
    }

    var body: some View {
        Button("Check for Updates…") {
            self.updater.showUpdaterIfNeeded()
        }
        .onReceive(self.updater.canCheckForUpdatesPublisher) { canCheckForUpdates in
            self.canCheckForUpdates = canCheckForUpdates
        }
        .disabled(!self.canCheckForUpdates)
    }
}
