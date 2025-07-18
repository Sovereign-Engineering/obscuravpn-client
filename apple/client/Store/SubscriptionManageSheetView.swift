import NetworkExtension
import SwiftUI

struct SubscriptionManageSheetView: View {
    let accountInfo: AccountInfo
    let storeKitModel: StoreKitModel
    let manager: NETunnelProviderManager?
    let openUrl: ((URL) -> Void)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AccountInfoOverviewView(accountInfo: self.accountInfo)

                if let openUrl {
                    PurchaseOptionsView(
                        accountInfo: self.accountInfo,
                        openUrl: openUrl,
                        storeKitModel: self.storeKitModel,
                        manager: self.manager
                    )
                }
            }
            .padding()
        }
    }
}
