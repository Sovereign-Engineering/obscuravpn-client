import Foundation

/// Unsafe callback wrapper, which allows calling the wrapped callback exactly once using only a pointer sized integer.
/// Calling it more than once is unsafe. Never calling it is a memory leak.
/// This is used to pass capturing closures across FFI boundaries.
class FfiCb<T> {
    typealias CallbackType = (T) -> Void
    private let callback: CallbackType

    private init(_ callback: @escaping CallbackType) {
        self.callback = callback
    }

    /// Get a pointer to the wrapped callback, which will prevent it being released until it is called.
    static func wrap(_ callback: @escaping CallbackType) -> UInt {
        let this = FfiCb(callback)
        return UInt(bitPattern: Unmanaged.passRetained(this).toOpaque())
    }

    /// Call the callback and then release it. The pointer will be unsafe to use after that.
    static func call(_ ptr: UInt, _ args: T) {
        let this = Unmanaged<FfiCb<T>>.fromOpaque(UnsafeRawPointer(bitPattern: ptr)!).takeRetainedValue()
        this.callback(args)
    }
}
