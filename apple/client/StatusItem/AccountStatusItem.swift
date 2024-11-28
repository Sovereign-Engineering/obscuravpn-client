import SwiftUI

func getAccountStatusItemText(_ accountDaysTillExpiry: AccountDaysTillExpiry) -> String {
    guard let daysToExpiry = accountDaysTillExpiry.days else { return "STOP IT" }
    if daysToExpiry > 3 {
        return "Account expires soon"
    }
    if daysToExpiry > 1 {
        return "Account expires in \(daysToExpiry) days"
    }
    if daysToExpiry == 1 {
        return "Accounts expires in in 1 day"
    }
    return "Account is expired"
}

struct AccountStatusItem: View {
    @ObservedObject var startupModel = StartupModel.shared
    @State var accountDaysTillExpiry = AccountDaysTillExpiry(days: nil)

    var body: some View {
        HStack {
            if self.startupModel.appState != nil && self.accountDaysTillExpiry.expiringSoon() {
                HStack {
                    VStack(alignment: .leading) {
                        Text(getAccountStatusItemText(self.accountDaysTillExpiry))
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    Spacer()
                }
                // this allows the Spacer to be clickable
                .contentShape(Rectangle())
                .padding(EdgeInsets(top: 5, leading: 14, bottom: 5, trailing: 12))
            }
        }
        .task {
            while true {
                do {
                    try await Task.sleep(seconds: 5)
                } catch {
                    return
                }
                self.accountDaysTillExpiry = self.startupModel.appState!.accountDaysTillExpiry
            }
        }
    }
}
