import NetworkExtension
import OSLog
import StoreKit
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "SubscriptionManageSheet")

private enum SheetType: Identifiable {
    case storeKitDebug
    case previewCarousel

    var id: Self { self }
}

struct SubscriptionManageSheet: View {
    let openUrl: ((URL) -> Void)?

    @StateObject private var viewModel: SubscriptionManageViewModel

    @State private var showStoreKitDebugUI = false
    @State private var showPreviewCarousel = false
    @State private var activeSheet: SheetType?
    @State private var showingDebugOptions = false

    @Environment(\.dismiss) private var dismiss

    // Initializer for production use with manager
    init(manager: NETunnelProviderManager, storeKitModel: StoreKitModel, openUrl: @escaping (URL) -> Void) {
        self.openUrl = openUrl
        self._viewModel = StateObject(wrappedValue: SubscriptionManageViewModel(manager: manager, storeKitModel: storeKitModel))
    }

    // Initializer for testing
    init(accountInfo: AccountInfo) {
        self.openUrl = nil
        self._viewModel = StateObject(wrappedValue: SubscriptionManageViewModel(manager: nil, accountInfo: accountInfo))
    }

    var body: some View {
        NavigationView {
            ZStack {
                if self.viewModel.accountInfo != nil {
                    SubscriptionManageSheetView(
                        viewModel: self.viewModel,
                        openUrl: self.openUrl
                    )
                    .opacity(self.viewModel.isLoading ? 0.3 : 1.0)
                    .animation(.easeInOut(duration: 0.2), value: self.viewModel.isLoading)
                } else if !self.viewModel.initialLoad {
                    ContentUnavailableView(
                        "No Account Information",
                        systemImage: "person.crop.circle.badge.exclamationmark",
                        description: Text("Unable to load account information at this time.")
                    )
                }

                if self.viewModel.isLoading && !self.viewModel.initialLoad {
                    ProgressView("Loading account information...")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .cornerRadius(12)
                        .transition(.opacity)
                        .animation(.easeInOut(duration: 0.3), value: self.viewModel.isLoading)
                }
            }
            .navigationTitle("Account Management")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Done") {
                        self.dismiss()
                    }
                }

                #if DEBUG
                    ToolbarItem(placement: .navigationBarTrailing) {
                        Button {
                            self.showingDebugOptions = true
                        } label: {
                            Image(systemName: "storefront.circle.fill")
                                .foregroundColor(.blue)
                                .font(.title2)
                        }
                        .confirmationDialog("Select Debug View", isPresented: self.$showingDebugOptions, titleVisibility: .visible) {
                            Button("StoreKit Debug") {
                                self.activeSheet = .storeKitDebug
                            }
                            Button("Preview Carousel") {
                                self.activeSheet = .previewCarousel
                            }
                            Button("Cancel", role: .cancel) {}
                        }
                    }
                #endif

                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        Task {
                            await self.viewModel.refresh()
                        }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .disabled(self.viewModel.isLoading)
                }
            }
        }
        .onAppear {
            // Load account info from manager
            Task {
                await self.viewModel.refresh()
            }
        }
        .refreshable {
            await self.viewModel.refresh()
        }
        .alert("Error", isPresented: self.$viewModel.showErrorAlert) {
            Button("OK") {}
        } message: {
            Text("Something went wrong")
        }
        .sheet(item: self.$activeSheet) { sheetType in
            switch sheetType {
            case .storeKitDebug:
                StoreDebugUI(storeKitModel: self.viewModel.storeKitModel, accountId: self.viewModel.accountInfo?.id)
            case .previewCarousel:
                SubscriptionManageSheetViewPreviewCarousel()
            }
        }
    }
}
