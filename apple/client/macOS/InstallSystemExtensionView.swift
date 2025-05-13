import SwiftUI

let macOS14DemoVideo = Bundle.main.url(forResource: "videos/macOS 14 System Extension Demo", withExtension: "mov")!
let macOS15DemoVideo = Bundle.main.url(forResource: "videos/macOS 15 System Extension Demo", withExtension: "mov")!

struct InstallSystemExtensionView: View {
    @ObservedObject var startupModel: StartupModel
    var subtext: String

    @Environment(\.openURL) private var openURL
    var neInit: NetworkExtensionInit? = nil

    var body: some View {
        ZStack {
            VStack {
                Spacer()
                Image("DecoPrimer")
                    .resizable()
                    .scaledToFit()
                    .frame(minWidth: 0, minHeight: 50)
            }
            VStack {
                Spacer()
                    .frame(minHeight: 20)
                HStack {
                    Spacer()
                    VStack(alignment: .leading, spacing: 10) {
                        Image("EmotePrimer")
                        Text("Allow System Extension")
                            .font(.title)
                        Text(self.subtext)
                            .font(.body)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                        if let neInit = self.neInit {
                            Button(action: neInit.continueAfterPriming) {
                                Text("Install Now")
                                    .font(.headline)
                                    .frame(width: 300)
                            }
                            .buttonStyle(NoFadeButtonStyle())
                        } else {
                            Button(action: {
                                if #available(macOS 15, *) {
                                    self.openURL(URLs.ExtensionSettings)
                                } else {
                                    self.openURL(URLs.PrivacySecurityExtensionSettings)
                                }
                            }) {
                                if #available(macOS 15, *) {
                                    Text("Open Login Items & Extensions Settings")
                                        .font(.headline)
                                        .frame(width: 300)
                                } else {
                                    Text("Open Privacy & Security Settings")
                                        .font(.headline)
                                        .frame(width: 300)
                                }
                            }
                            .buttonStyle(NoFadeButtonStyle())
                        }
                    }
                    .frame(width: 350)
                    .padding(.leading, 50)
                    Spacer()
                    if #available(macOS 15, *) {
                        LoopingVideoPlayer(url: macOS15DemoVideo, width: 360, height: 410)
                    } else {
                        LoopingVideoPlayer(url: macOS14DemoVideo, width: 360, height: 410)
                    }
                    Spacer()
                }
                Spacer()
                    .frame(minHeight: 50)
            }
            VStack(alignment: .trailing) {
                Spacer()
                HStack(alignment: .bottom) {
                    Spacer()
                    if #available(macOS 14.0, *) {
                        HelpLink(destination: URLs.SystemExtensionHelp)
                            .padding(.bottom, 2)
                    } else {
                        Button {
                            self.openURL(URLs.SystemExtensionHelp)
                        } label: {
                            Image(systemName: "questionmark.circle.fill")
                                .font(.system(size: 19))
                                .foregroundStyle(.white, .gray.opacity(0.4))
                        }
                        .buttonStyle(.plain)
                        .padding(.bottom, 2)
                        .padding(.trailing, 2)
                    }
                }
                .padding()
            }
        }
    }
}
