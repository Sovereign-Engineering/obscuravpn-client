import SwiftUI

struct HyperlinkButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundColor(self.isEnabled ? .blue : .blue.opacity(0.5))
    }
}
