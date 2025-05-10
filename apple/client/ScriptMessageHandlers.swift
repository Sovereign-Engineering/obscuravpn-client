import Foundation
import OSLog
import WebKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Webview")

class CommandHandler: NSObject, WKScriptMessageHandlerWithReply {
    static var shared = CommandHandler()

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let commandJson = message.body as? String else {
            replyHandler(nil, "command not a string")
            return
        }
        let commandJsonBytes: Data! = commandJson.data(using: .utf8)
        guard let command = try? JSONDecoder().decode(Command.self, from: commandJsonBytes) else {
            replyHandler(nil, "decoding command failed")
            return
        }
        Task {
            do {
                let response = try await handleWebViewCommand(command: command)
                replyHandler(response, nil)
            } catch let error as String {
                replyHandler(nil, error)
            }
        }
    }
}

class ErrorHandler: NSObject, WKScriptMessageHandler {
    static var shared = ErrorHandler()
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let string = message.body as? String else {
            logger.error("webview error was not a string: \(debugFormat(message.body), privacy: .public)")
            return
        }
        logger.info("error: \(string, privacy: .public)")
    }
}

class LogHandler: NSObject, WKScriptMessageHandler {
    // handles console.log, console.info, console.error (log will include the level)
    static var shared = LogHandler()
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let string = message.body as? String else {
            logger.error("webview log was not a string: \(debugFormat(message.body), privacy: .public)")
            return
        }
        logger.info("\(string, privacy: .public)")
    }
}
