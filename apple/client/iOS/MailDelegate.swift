import MessageUI
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Mail")

class MailDelegate: NSObject, MFMailComposeViewControllerDelegate {
    func mailComposeController(_ controller: MFMailComposeViewController, didFinishWith result: MFMailComposeResult, error: Error?) {
        switch result {
        case MFMailComposeResult.cancelled:
            logger.debug("Cancelled mail")
        case MFMailComposeResult.saved:
            logger.debug("Saved mail")
        case MFMailComposeResult.sent:
            logger.info("Sent mail successfully")
        case MFMailComposeResult.failed:
            logger.error("Failed to send mail: \(error?.localizedDescription, privacy: .public)")
        default:
            break
        }
        controller.dismiss(animated: true)
    }
}
