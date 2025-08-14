import OSLog
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Notifications")

enum NotificationId: String {
    case autoConnectFailed = "obscura-auto-connect-failed"
    case connectFailed = "obscura-connect-failed"
}

func displayNotification(
    _ identifier: NotificationId,
    _ content: UNMutableNotificationContent
) {
    Task {
        do {
            let center = UNUserNotificationCenter.current()
            let granted = try await center.requestAuthorization(
                options: [.alert, .badge, .sound]
            )

            if !granted {
                logger.warning("Notifications blocked.")
                return
            }

            try await center.add(
                UNNotificationRequest(
                    identifier: identifier.rawValue,
                    content: content,
                    trigger: nil
                )
            )
        } catch {
            logger.error("Failed to display notification: \(error, privacy: .public)")
        }
    }
}

func notifyConnectError(_ error: Error) {
    let content = UNMutableNotificationContent()
    if error.localizedDescription == "accountExpired" {
        content.body = "Your account has expired."
    } else {
        content.body = "An error occurred while connecting to the tunnel."
    }
    content.title = "Tunnel failed to connect"
    content.interruptionLevel = .active
    content.sound = UNNotificationSound.defaultCritical
    displayNotification(
        .connectFailed,
        content,
    )
}
