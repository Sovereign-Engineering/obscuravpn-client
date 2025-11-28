import NetworkExtension
import SwiftUI

struct SubscriptionManageSheetView: View {
    @ObservedObject var viewModel: SubscriptionManageViewModel
    let openUrl: ((URL) -> Void)?

    init(
        viewModel: SubscriptionManageViewModel,
        openUrl: ((URL) -> Void)?
    ) {
        self.viewModel = viewModel
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

                if self.viewModel.storeKitPurchasedAwaitingServerAck {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .padding()
        }
    }
}
