import Foundation
import libobscuravpn_client
import Network

func ffiInitializeSystemLogging(_ logDir: String?) -> OpaquePointer? {
    let logDir: String = logDir ?? ""
    let logFlushGuard = logDir.withFfiStr { ffiLogDir in
        libobscuravpn_client.initialize_apple_system_logging(ffiLogDir)
    }
    return logFlushGuard
}

class RustFfi {
    private let ptr: OpaquePointer

    init(configDir: String, userAgent: String, logFlushGuard: OpaquePointer?, _ receiveCallback: @convention(c) (FfiBytes) -> Void, _ setNetworkConfigCallback: @convention(c) (FfiBytes, UnsafeMutableRawPointer?, (@convention(c) (UnsafeMutableRawPointer?, Bool) -> Void)?) -> Void) {
        let wgSecretKey = keychainGetWgSecretKey() ?? Data()
        let p = configDir.withFfiStr { ffiConfigDir in
            userAgent.withFfiStr { ffiUserAgent in
                wgSecretKey.withFfiBytes { ffiWgSecretKey in
                    libobscuravpn_client.initialize(ffiConfigDir, ffiUserAgent, ffiWgSecretKey, receiveCallback, setNetworkConfigCallback, keychainSetWgSecretKeyCallback, logFlushGuard)
                }
            }
        }
        self.ptr = p!
    }

    func jsonManagerCmd(_ jsonCmd: Data) async -> NeManagerCmdResult {
        return await withCheckedContinuation { continuation in
            let context = FfiCb.wrap { (ok_json: FfiStr, err: FfiStr) in
                if let err = err.nonEmptyString() {
                    continuation.resume(returning: .error(err))
                    return
                }
                continuation.resume(returning: .ok_json(ok_json.string()))
            }
            jsonCmd.withFfiBytes {
                libobscuravpn_client.json_ffi_cmd(self.ptr, context, $0) { FfiCb.call($0, ($1, $2)) }
            }
        }
    }

    func sendPacket(_ packet: Data) {
        packet.withFfiBytes {
            libobscuravpn_client.send_packet(self.ptr, $0)
        }
    }

    func setNetworkInterface(_ networkInterface: (Int, String)?) {
        if let (index, name): (Int, String) = networkInterface {
            if index <= 0 || Int64(index) > Int64(UInt32.max) {
                ffiLog(.Error, "network interface index out of range \(index)")
                "".withFfiStr { ffiEmptyName in
                    libobscuravpn_client.set_network_interface(self.ptr, 0, ffiEmptyName)
                }
            } else {
                name.withFfiStr { ffiName in
                    libobscuravpn_client.set_network_interface(self.ptr, UInt32(index), ffiName)
                }
            }
        } else {
            "".withFfiStr { ffiEmptyName in
                libobscuravpn_client.set_network_interface(self.ptr, 0, ffiEmptyName)
            }
        }
    }

    func wake() {
        libobscuravpn_client.wake(self.ptr)
    }
}

enum LogLevel: UInt8 {
    case Trace
    case Debug
    case Info
    case Warn
    case Error
}

func ffiLog(
    _ level: LogLevel,
    _ message: String,
    fileID: String = #fileID,
    function: String = #function,
    line: Int = #line
) {
    message.withFfiStr { ffiMessage in
        fileID.withFfiStr { ffiFileID in
            function.withFfiStr { ffiFunction in
                libobscuravpn_client.forward_log(level.rawValue, ffiMessage, ffiFileID, ffiFunction, line)
            }
        }
    }
}

private func keychainSetWgSecretKeyCallback(key: FfiBytes) -> Bool {
    ffiLog(.Info, "keychainSetWgSecretKeyCallback entry")
    let ret = keychainSetWgSecretKey(key.data())
    if !ret {
        ffiLog(.Info, "keychainSetWgSecretKey returned false")
    }
    ffiLog(.Info, "keychainSetWgSecretKeyCallback exit")
    return ret
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
