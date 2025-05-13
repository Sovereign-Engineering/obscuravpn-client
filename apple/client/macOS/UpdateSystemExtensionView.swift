import SwiftUI

struct UpdateSystemExtensionView: View {
    @ObservedObject var startupModel: StartupModel
    var subtext: String

    @Environment(\.openURL) private var openURL
    var neInit: NetworkExtensionInit

    var body: some View {
        Spacer()
            .frame(height: 60)
        // extensions symbol for macOS <= 15
        // coincidentally used for the network extensions symbol on macOS 15
        Image(systemName: "puzzlepiece.extension.fill")
            .font(.system(size: 48))
            .padding()
        Text("System Extension Update Required")
            .font(.title)
        Text(self.subtext)
            .font(.body)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(width: 350)
            .padding()

        Button(action: self.neInit.continueAfterPriming) {
            Text("Disconnect and Update")
                .font(.headline)
                .frame(width: 300)
        }
        .buttonStyle(NoFadeButtonStyle())
    }
}
