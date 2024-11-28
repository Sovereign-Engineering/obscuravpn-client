import OSLog
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Notifications")

func displayNotification(
    _ req: UNNotificationRequest
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

            try await center.add(req)
        } catch {
            logger.error("Failed to display notification: \(error, privacy: .public)")
        }
    }
}
