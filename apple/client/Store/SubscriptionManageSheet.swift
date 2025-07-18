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
    let manager: NETunnelProviderManager?
    let openUrl: ((URL) -> Void)?

    @State private var accountInfo: AccountInfo?
    let storeKitModel: StoreKitModel
    @State private var isLoading = false
    @State private var initialLoad = true
    @State private var showErrorAlert = false
    @State private var showStoreKitDebugUI = false
    @State private var showPreviewCarousel = false
    @State private var activeSheet: SheetType?
    @State private var showingDebugOptions = false

    @Environment(\.dismiss) private var dismiss

    // Initializer for production use with manager
    init(manager: NETunnelProviderManager, storeKitModel: StoreKitModel, openUrl: @escaping (URL) -> Void) {
        self.manager = manager
        self.openUrl = openUrl
        self.storeKitModel = storeKitModel
    }

    // Initializer for testing
    init(accountInfo: AccountInfo) {
        self.manager = nil
        self.openUrl = nil
        self.storeKitModel = StoreKitModel()
        self._accountInfo = State(initialValue: accountInfo)
    }

    private var canRefresh: Bool {
        self.manager != nil
    }

    var body: some View {
        NavigationView {
            ZStack {
                if let accountInfo = accountInfo {
                    SubscriptionManageSheetView(
                        accountInfo: accountInfo,
                        storeKitModel: self.storeKitModel,
                        manager: self.manager,
                        openUrl: self.openUrl
                    )
                    .opacity(self.isLoading ? 0.3 : 1.0)
                    .animation(.easeInOut(duration: 0.2), value: self.isLoading)
                } else if !self.initialLoad {
                    ContentUnavailableView(
                        "No Account Information",
                        systemImage: "person.crop.circle.badge.exclamationmark",
                        description: Text("Unable to load account information at this time.")
                    )
                }

                if self.isLoading && !self.initialLoad {
                    ProgressView("Loading account information...")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .cornerRadius(12)
                        .transition(.opacity)
                        .animation(.easeInOut(duration: 0.3), value: self.isLoading)
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
                            await self.loadAccountInfo()
                        }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .disabled(self.isLoading || !self.canRefresh)
                }
            }
        }
        .onAppear {
            if self.manager != nil {
                // Load account info from manager
                Task {
                    await self.loadAccountInfo()
                }
            }
        }
        .refreshable {
            if self.canRefresh {
                await self.loadAccountInfo()
            }
        }
        .alert("Error", isPresented: self.$showErrorAlert) {
            Button("OK") {}
            Button("Retry") {
                Task {
                    await self.loadAccountInfo()
                }
            }
        } message: {
            Text("Something went wrong")
        }
        .sheet(item: self.$activeSheet) { sheetType in
            switch sheetType {
            case .storeKitDebug:
                StoreDebugUI(storeKitModel: self.storeKitModel)
            case .previewCarousel:
                SubscriptionManageSheetViewPreviewCarousel()
            }
        }
    }

    @MainActor
    private func loadAccountInfo() async {
        guard let manager = manager else {
            logger.warning("Attempted to load account info without manager")
            return
        }

        self.isLoading = true
        do {
            let accountInfo = try await getAccountInfo(manager)
            self.accountInfo = accountInfo
            self.isLoading = false
        } catch {
            logger.error("Failed to load account info: \(error, privacy: .public)")
            self.showErrorAlert = true
            self.isLoading = false
        }
    }
}
