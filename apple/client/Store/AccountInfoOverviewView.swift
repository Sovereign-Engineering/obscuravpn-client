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

            init(title: String, importance: Importance, data: String, dataBolded: Bool = false, dataColor: Color? = nil) {
                self.title = title
                self.importance = importance
                self.data = data
                self.dataBolded = dataBolded
                self.dataColor = dataColor
            }

            init(title: String, importance: Importance, date: Date, dataBolded: Bool = false, dataColor: Color? = nil) {
                self.title = title
                self.importance = importance
                self.data = date.formatted(date: .abbreviated, time: .omitted)
                self.dataBolded = dataBolded
                self.dataColor = dataColor
            }

            init(title: String) {
                self.title = title
                self.importance = .high
                self.data = ""
                self.dataBolded = false
                self.dataColor = nil
            }
        }

        struct Section {
            let rows: [Row]

            init(rows: [Row]) {
                self.rows = rows
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ForEach(Array(self.configuration.sections.enumerated()), id: \.offset) { sectionIndex, section in
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(section.rows.enumerated()), id: \.offset) { rowIndex, row in
                        HStack {
                            Text(row.title)
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

    let accountInfo: AccountInfo

    public init(accountInfo: AccountInfo) {
        self.accountInfo = accountInfo
    }

    public var body: some View {
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

        let statusSection = Section(rows: [
            Row(
                title: "Status",
                importance: .high,
                data: self.accountInfo.active ? "Active" : "Inactive",
                dataColor: self.accountInfo.active ? .green : .red
            ),
        ])
        sections.append(statusSection)

        if let topUp = self.accountInfo.topUp {
            let topUpSection = Section(rows: [
                Row(title: "Top Up"),
                Row(
                    title: "Expiration Date",
                    importance: .medium,
                    date: Date(timeIntervalSince1970: TimeInterval(topUp.creditExpiresAt))
                ),
            ])
            sections.append(topUpSection)
        }

        if let stripeSubscription = self.accountInfo.stripeSubscription {
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
                    date: Date(timeIntervalSince1970: TimeInterval(stripeSubscription.currentPeriodStart))
                ),
                Row(
                    title: "Period End",
                    importance: .medium,
                    date: Date(timeIntervalSince1970: TimeInterval(stripeSubscription.currentPeriodEnd))
                ),
                Row(
                    title: "Cancel at Period End",
                    importance: .medium,
                    data: stripeSubscription.cancelAtPeriodEnd ? "Yes" : "No"
                ),
            ])
            sections.append(stripeSection)
        }

        if let appleSubscription = self.accountInfo.appleSubscription {
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
                    date: Date(timeIntervalSince1970: TimeInterval(appleSubscription.renewalDate))
                ),
            ])
            sections.append(appleSection)
        }

        return SectionedTableInfoView.Configuration(sections: sections)
    }

    private var formattedAccountId: String? {
        // Every 4th character, add a dash
        self.accountInfo.id.enumerated().map { index, char in
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
