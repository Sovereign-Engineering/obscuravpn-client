import SwiftUI

struct RegisterLoginItemView: View {
    var value: ObservableValue<Bool>
    @Environment(\.openURL) private var openURL
    @State private var isRegistering = false

    var body: some View {
        Image(systemName: "desktopcomputer.and.arrow.down")
            .font(.system(size: 48))
            .symbolRenderingMode(.palette)
            .foregroundStyle(.white, .blue)
            .padding()
            .buttonStyle(.plain)

        Text("Open at Login")
            .font(.title)

        Text("Do you want Obscura VPN to open automatically when you log in?")
            .font(.body)
            .multilineTextAlignment(.center)
            .padding()

        if self.isRegistering {
            ProgressView()
        } else {
            Button(action: { self.value.publish(true) }) {
                Text("Yes")
                    .font(.headline)
                    .frame(width: 300)
            }
            .buttonStyle(NoFadeButtonStyle())

            Button(action: { self.value.publish(false) }) {
                Text("No")
                    .frame(width: 300)
            }
            .buttonStyle(NoFadeButtonStyle(backgroundColor: Color(.darkGray)))
        }
    }
}

enum StartupStatus {
    case initial
    case networkExtensionInit(NetworkExtensionInit, NetworkExtensionInitStatus)
    case tunnelProviderInit(TunnelProviderInit, TunnelProviderInitStatus)
    case askToRegisterLoginItem(ObservableValue<Bool>)
    case ready
}
