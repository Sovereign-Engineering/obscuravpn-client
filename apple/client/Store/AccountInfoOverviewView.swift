import SwiftUI

struct SectionedTableInfoView: View {
    let configuration: Configuration

    struct Configuration {
        let sections: [Section]

        init(sections: [Section]) {
            self.sections = sections
        }

        struct Row {
            enum Importance {
                case high
                case medium
                case low
            }

            let title: String
            let importance: Importance
            let data: String
            let dataBolded: Bool
            let dataColor: Color?
            let loading: Bool

            init(title: String, importance: Importance, data: String, dataBolded: Bool = false, dataColor: Color? = nil, loading: Bool = false) {
                self.title = title
                self.importance = importance
                self.data = data
                self.dataBolded = dataBolded
                self.dataColor = dataColor
                self.loading = loading
            }

            init(title: String, importance: Importance, date: Date, dataBolded: Bool = false, dataColor: Color? = nil, loading: Bool = false) {
                self.title = title
                self.importance = importance
                self.data = date.formatted(date: .abbreviated, time: .omitted)
                self.dataBolded = dataBolded
                self.dataColor = dataColor
                self.loading = loading
            }

            init(title: String) {
                self.title = title
                self.importance = .high
                self.data = ""
                self.dataBolded = false
                self.dataColor = nil
                self.loading = false
            }
        }

        struct Section {
            let rows: [Row]

            init(rows: [Row]) {
                self.rows = rows
            }
        }
    }

    private struct LoadingDots: View {
        @State private var dotCount = 1
        let timer = Timer.publish(every: 0.6, on: .main, in: .common).autoconnect()

        var body: some View {
            Group {
                if self.dotCount != 1 {
                    Text(String(repeating: ".", count: self.dotCount - 1))
                } else {
                    Text("")
                }
            }
            .onReceive(self.timer) { _ in
                self.dotCount = (self.dotCount % 4) + 1
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ForEach(Array(self.configuration.sections.enumerated()), id: \.offset) { sectionIndex, section in
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(section.rows.enumerated()), id: \.offset) { rowIndex, row in
                        HStack(spacing: 0) {
                            Group {
                                Text(row.title)
                                if row.loading {
                                    LoadingDots()
                                }
                            }
                            .font(row.importance == .high ? .headline : .subheadline)
                            .fontWeight(row.importance == .high ? .bold : .regular)
                            .foregroundColor(row.importance == .high ? .primary : .secondary)

                            Spacer()

                            Text(row.data)
                                .font(row.importance == .high ? .headline : .subheadline)
                                .fontWeight(row.dataBolded ? .bold : .regular)
                                .foregroundColor(self.rightSideColor(for: row))
                                .textSelection(.enabled)
                        }
                    }
                }

                if sectionIndex < self.configuration.sections.count - 1 {
                    Divider()
                }
            }
        }
    }

    private func rightSideColor(for row: Configuration.Row) -> Color {
        if let dataColor = row.dataColor {
            return dataColor
        }

        switch row.importance {
        case .high:
            return .secondary
        case .medium:
            return .primary
        case .low:
            return .secondary
        }
    }
}

// MARK: - AccountInfoOverviewView

struct AccountInfoOverviewView: View {
    typealias Row = SectionedTableInfoView.Configuration.Row
    typealias Section = SectionedTableInfoView.Configuration.Section

    @ObservedObject var viewModel: SubscriptionManageViewModel
    @ObservedObject var storeKitModel: StoreKitModel

    init(
        viewModel: SubscriptionManageViewModel
    ) {
        self.viewModel = viewModel
        self.storeKitModel = viewModel.storeKitModel
    }

    var body: some View {
        SectionedTableInfoView(configuration: self.configuration)
    }

    private var configuration: SectionedTableInfoView.Configuration {
        var sections: [Section] = []

        let accountSection = Section(rows: [
            Row(
                title: "Account ID",
                importance: .high,
                data: self.formattedAccountId ?? "No account ID"
            ),
        ])
        sections.append(accountSection)

        if let accountInfo = viewModel.accountInfo {
            sections.append(Section(rows: self.accountStatusSection))

            if let topUp = accountInfo.topUp {
                let topUpSection = Section(rows: [
                    Row(title: "Top Up"),
                    Row(
                        title: "Expiration Date",
                        importance: .medium,
                        date: topUp.creditExpiresAtDate
                    ),
                ])
                sections.append(topUpSection)
            }

            if let stripeSubscription = accountInfo.stripeSubscription {
                let stripeSection = Section(rows: [
                    Row(title: "Subscribed on Obscura.net"),
                    Row(
                        title: "Status",
                        importance: .medium,
                        data: stripeSubscription.status.rawValue.capitalized,
                        dataColor: self.stripeStatusColor(stripeSubscription.status)
                    ),
                    Row(
                        title: "Source",
                        importance: .medium,
                        data: "obscura.net"
                    ),
                    Row(
                        title: "Period Start",
                        importance: .medium,
                        date: stripeSubscription.currentPeriodStartDate
                    ),
                    Row(
                        title: "Period End",
                        importance: .medium,
                        date: stripeSubscription.currentPeriodEndDate
                    ),
                    Row(
                        title: "Cancel at Period End",
                        importance: .medium,
                        data: stripeSubscription.cancelAtPeriodEnd ? "Yes" : "No"
                    ),
                ])
                sections.append(stripeSection)
            }

            if let appleSubscription = accountInfo.appleSubscription {
                let appleSection = Section(rows: [
                    Row(title: "Subscription"),
                    Row(
                        title: "Status",
                        importance: .medium,
                        data: appleSubscription.subscriptionStatus.description,
                        dataColor: self.appleSubscriptionStatusColor(appleSubscription.subscriptionStatus)
                    ),
                    Row(
                        title: "Source",
                        importance: .medium,
                        data: "App Store"
                    ),
                    Row(
                        title: "Auto-Renewal",
                        importance: .medium,
                        data: appleSubscription.autoRenewalStatus ? "Enabled" : "Disabled"
                    ),
                    Row(
                        title: "Renewal Date",
                        importance: .medium,
                        date: appleSubscription.renewalDate
                    ),
                ])
                sections.append(appleSection)
            }
        }

        return SectionedTableInfoView.Configuration(sections: sections)
    }

    private var accountStatusSection: [Row] {
        guard let accountInfo = viewModel.accountInfo else { return [] }

        var build: [Row] = []

        // Overall status
        build.append(Row(
            title: "Status",
            importance: .high,
            data: accountInfo.active ? "Active" : "Inactive",
            dataColor: accountInfo.active ? .green : .red
        ))

        if self.viewModel.storeKitPurchasedAwaitingServerAck {
            let hasStorekit = self.storeKitModel.hasActiveMonthlySubscription
            if !accountInfo.active || hasStorekit {
                build.append(Row(
                    title: "Subscription Status",
                    importance: .low,
                    data: hasStorekit ? "Paid" : "Unsubscribed",
                    dataColor: hasStorekit ? .yellow : .black
                ))
            }

            build.append(Row(
                title: "Please wait",
                importance: .low,
                data: "Preparing VPN on backend",
                loading: true
            ))
        }

        return build
    }

    private var formattedAccountId: String? {
        // Every 4th character, add a dash
        self.viewModel.accountInfo?.id.enumerated().map { index, char in
            index > 0 && index % 4 == 0 ? "-\(char)" : String(char)
        }.joined()
    }

    private func appleSubscriptionStatusColor(_ status: AppleSubscriptionInfo.Status) -> Color {
        switch status {
        case .active:
            return .green
        case .gracePeriod, .billingRetry:
            return .orange
        case .expired, .revoked:
            return .red
        }
    }

    private func stripeStatusColor(_ status: StripeSubscriptionStatus) -> Color {
        switch status {
        case .active, .trialing:
            return .green
        case .pastDue, .incomplete, .paused:
            return .yellow
        case .canceled, .unpaid, .incompleteExpired:
            return .red
        }
    }
}
