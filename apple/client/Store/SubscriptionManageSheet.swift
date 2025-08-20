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
    @Environment(\.scenePhase) private var scenePhase

    let openUrl: ((URL) -> Void)?

    @StateObject private var viewModel: SubscriptionManageViewModel

    @State private var activeSheet: SheetType?
    @State private var showingDebugOptions = false

    @State private var longLoadDetected: Bool = false
    @State private var longLoadingTask: Task<Void, Error>? = nil
    var isLongLoading: Bool {
        // Failsafe so that isLongLoading is never true when viewModel.isLoading is false
        return self.longLoadDetected && self.viewModel.isLoading
    }

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
                    .opacity(self.isLongLoading ? 0.3 : 1.0)
                    .animation(.easeInOut(duration: 0.2), value: self.isLongLoading)
                } else if !self.viewModel.initialLoad {
                    ContentUnavailableView(
                        "No Account Information",
                        systemImage: "person.crop.circle.badge.exclamationmark",
                        description: Text("Unable to load account information at this time.")
                    )
                }

                if self.viewModel.isLoading {
                    ProgressView(self.isLongLoading ? "Loading account information..." : "")
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
                    Button("Close") {
                        self.dismiss()
                    }
                }

                if self.canSeeDebugItems {
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
                }

                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        Task {
                            await self.viewModel.refresh(userOriginated: true)
                        }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .disabled(self.viewModel.isLoading)
                }
            }
        }
        .onChange(of: self.scenePhase) { _, newPhase in
            if newPhase == .active {
                Task {
                    await self.viewModel.refresh(userOriginated: false)
                }
            }
        }
        .refreshable {
            await self.viewModel.refresh(userOriginated: true)
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
        .onChange(of: self.viewModel.isLoading) { _, newValue in
            self.longLoadingTask?.cancel()
            if !newValue {
                self.longLoadDetected = false
            } else {
                self.longLoadingTask = Task {
                    try await Task.sleep(for: .seconds(1))
                    try Task.checkCancellation()
                    self.longLoadDetected = true
                }
            }
        }
    }

    var canSeeDebugItems: Bool {
        return self.viewModel.debugGestureActivated
    }
}
