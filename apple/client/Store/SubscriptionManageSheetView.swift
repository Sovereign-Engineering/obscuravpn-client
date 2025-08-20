import NetworkExtension
import SwiftUI

struct SubscriptionManageSheetView: View {
    @ObservedObject var viewModel: SubscriptionManageViewModel
    @ObservedObject var storeKitModel: StoreKitModel
    let openUrl: ((URL) -> Void)?

    init(
        viewModel: SubscriptionManageViewModel,
        openUrl: ((URL) -> Void)?
    ) {
        self.viewModel = viewModel
        self.storeKitModel = viewModel.storeKitModel
        self.openUrl = openUrl
    }

    var body: some View {
        self.overviewAndPurchaseOptions
            .safeAreaInset(edge: .bottom) {
                if !self.viewModel.storeKitPurchasedAwaitingServerAck, let openUrl {
                    PurchaseOptionsView(
                        openUrl: openUrl,
                        viewModel: self.viewModel
                    )
                    .padding()
                    .padding(.bottom, 20)
                }
            }
    }

    var overviewAndPurchaseOptions: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                AccountInfoOverviewView(viewModel: self.viewModel)
                    .onTapGesture(count: 5) {
                        self.viewModel.debugGestureActivated = true
                    }

                if self.viewModel.storeKitPurchasedAwaitingServerAck {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .padding()
        }
    }
}
