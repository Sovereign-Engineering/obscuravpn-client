import Cocoa
import OSLog
import SwiftUI
import UserNotifications

func getExpiredInDaysText(_ days: UInt64) -> String {
    if days > 1 {
        return "in \(days) days"
    }
    if days == 1 {
        return "in \(days) day"
    }
    return "in < 1 day"
}

struct StatusItemAccount: View {
    @Environment(\.openURL) private var openURL
    var account: AccountStatus

    var body: some View {
        VStack {
            if self.account.expiringSoon() {
                Label {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Fund your account...")
                                .font(.system(size: 13))
                            HStack {
                                if self.account.isActive() {
                                    Text("Account expires soon")
                                        .foregroundStyle(.secondary)
                                } else {
                                    Text("Account is expired")
                                        .foregroundStyle(.red)
                                }
                                Spacer()
                                Text(self.account.isActive() ? getExpiredInDaysText(self.account.daysUntilExpiry()!) : "        ")
                                    .foregroundStyle(.tertiary)
                                    .fixedSize()
                                    .frame(minWidth: 50)
                            }
                            .font(.subheadline)
                        }.fixedSize(horizontal: true, vertical: false)
                        Spacer()
                    }
                } icon: {
                    Image(systemName: "exclamationmark.arrow.circlepath")
                }
                // this allows the Spacer to be clickable
                .contentShape(Rectangle())
                .padding(EdgeInsets(top: 2, leading: 14, bottom: 2, trailing: 12))
            }
        }
        .onTapGesture {
            self.openURL(URLs.AppAccountPage)
        }
    }
}
