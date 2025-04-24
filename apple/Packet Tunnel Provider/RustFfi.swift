import Foundation
import libobscuravpn_client
import Network
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Rust FFI")

func ffiInitialize(configDir: String, userAgent: String, _ receiveCallback: (@convention(c) (FfiBytes) -> Void)!) {
    libobscuravpn_client.initialize_macos_system_logging()
    configDir.withFfiStr { ffiConfigDir in
        userAgent.withFfiStr { ffiUserAgent in
            libobscuravpn_client.initialize(ffiConfigDir, ffiUserAgent, receiveCallback)
        }
    }
}

func ffiJsonManagerCmd(_ jsonCmd: Data) async -> NeManagerCmdResult {
    return await withCheckedContinuation { continuation in
        let context = FfiCb.wrap { (ok_json: FfiStr, err: FfiStr) in
            if let err = err.nonEmptyString() {
                continuation.resume(returning: .error(err))
                return
            }
            continuation.resume(returning: .ok_json(ok_json.string()))
        }
        jsonCmd.withFfiBytes {
            libobscuravpn_client.json_ffi_cmd(context, $0) { FfiCb.call($0, ($1, $2)) }
        }
    }
}

extension String {
    func withFfiStr<R>(_ body: (libobscuravpn_client.FfiStr) -> R) -> R {
        self.data(using: .utf8)!.withFfiBytes {
            let ffiStr = libobscuravpn_client.FfiStr(bytes: $0)
            return body(ffiStr)
        }
    }
}

extension FfiStr {
    func string() -> String {
        String(decoding: self.bytes.data(), as: UTF8.self)
    }

    func nonEmptyString() -> String? {
        let s = self.string()
        return s.isEmpty ? nil : s
    }
}

extension Data {
    func withFfiBytes<R>(_ body: (libobscuravpn_client.FfiBytes) -> R) -> R {
        self.withUnsafeBytes {
            let ffiBytes = libobscuravpn_client.FfiBytes(buffer: $0.baseAddress, len: UInt($0.count))
            return body(ffiBytes)
        }
    }
}

extension FfiBytes {
    func data() -> Data {
        Data(bytes: self.buffer, count: Int(self.len))
    }
}
