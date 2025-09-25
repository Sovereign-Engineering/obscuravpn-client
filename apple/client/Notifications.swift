import OSLog
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Notifications")

func displayNotification(
    _ identifier: NotificationId,
    _ content: UNMutableNotificationContent
) {
    Task {
        do {
            let granted = await requestNotificationAuthorization()
            if !granted {
                return
            }

            try await UNUserNotificationCenter.current().add(
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

func requestNotificationAuthorization() async -> Bool {
    do {
        if try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound]) {
            logger.info("Notifications authorization granted.")
            return true
        } else {
            logger.warning("Notifications blocked.")
        }
    } catch {
        logger.error("Notification authorization request failed: \(error)")
    }
    return false
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
    displayNotification(.connectFailed, content)
}
