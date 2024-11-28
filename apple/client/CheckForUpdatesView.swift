import Sparkle
import SwiftUI

/**
 This is the view for the Check for Updates menu item

 Note this intermediate view is necessary for the disabled state on the menu item to work properly before Monterey.
 See https://stackoverflow.com/questions/68553092/menu-not-updating-swiftui-bug for more info.
 **/
struct CheckForUpdatesView: View {
    @ObservedObject private var viewModel: CheckForUpdatesViewModel

    private let updater: SPUUpdater

    init(updater: SPUUpdater) {
        self.updater = updater

        self.viewModel = .init(updater: updater)
    }

    var body: some View {
        Button("Check for Updates…", action: self.updater.checkForUpdates)
            .disabled(!self.viewModel.canCheckForUpdates)
    }
}

/**
 This view model class publishes when new updates can be checked by the user.
 **/
final class CheckForUpdatesViewModel: ObservableObject {
    @Published var canCheckForUpdates = false

    init(updater: SPUUpdater) {
        updater.publisher(for: \.canCheckForUpdates)
            .assign(to: &self.$canCheckForUpdates)
    }
}
