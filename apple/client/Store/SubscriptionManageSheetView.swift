import NetworkExtension
import SwiftUI

struct SubscriptionManageSheetView: View {
    let accountInfo: AccountInfo
    @ObservedObject var storeKitModel: StoreKitModel
    let manager: NETunnelProviderManager?
    let openUrl: ((URL) -> Void)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AccountInfoOverviewView(
                    accountInfo: self.accountInfo,
                    storeKitSubscriptionActive: self.storeKitModel.hasActiveMonthlySubscription
                )

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
