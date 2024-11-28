import Foundation
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "Concurrency")

/// Track a set of callbacks that can be triggered.
class Callbacks<V> {
    typealias CallbackId = ObjectId<(V) -> Void>

    private var pending: Set<CallbackId> = []

    /// Add a callback to the queue.
    ///
    /// The return value can be used to cancel the callback.
    @discardableResult
    func add(_ f: @escaping (V) -> Void) -> CallbackId {
        let cb = ObjectId(f)
        self.pending.insert(cb)
        return cb
    }

    /// Cancel a scheduled callback.
    ///
    /// Does nothing if the callback has already been executed or removed.
    func remove(_ cb: CallbackId) {
        self.pending.remove(cb)
    }

    /// Trigger all callbacks.
    ///
    /// This triggers all callbacks and clears the queue.
    func dispatch(_ value: V) {
        // Swap first to be re-entrant.
        let pending = self.pending
        self.pending = []

        for cb in pending {
            cb.value(value)
        }
    }
}

/// A tool for tracking outstanding tasks.
///
/// Note: If `TaskGroup` is suitable for your use case you should prefer that. (https://developer.apple.com/documentation/swift/taskgroup)
///
/// This type is internally synchronized and all methods are safe to be called concurrently.
class PendingTasks {
    private var lock = NSLock()
    private var count: UInt64 = 0
    private var waiting = Callbacks<Void>()

    init() {}

    /// Record that a task has been started.
    func start(tasks: UInt64 = 1) {
        self.lock.withLock {
            self.count += tasks
        }
    }

    /// Record that a task has completed.
    func complete(tasks: UInt64 = 1) {
        self.lock.withLock {
            if tasks > self.count {
                logger.error("More tasks completed (\(tasks, privacy: .public)) than running (\(self.count, privacy: .public))")
                self.count = 0
            } else {
                self.count -= tasks
            }

            if self.count == 0 {
                self.waiting.dispatch(())
            }
        }
    }

    /// Wait until there are no tasks running.
    ///
    /// This will return the first time there are no outstanding tasks, or immediately if there are currently none. Tasks that are added while waiting will also be waited for.
    func waitForAll() async {
        await withCheckedContinuation { continuation in
            self.lock.withLock {
                if self.count == 0 {
                    continuation.resume(returning: ())
                } else {
                    self.waiting.add {
                        continuation.resume(returning: ())
                    }
                }
            }
        }
    }
}

struct TimeoutError: Error {
    var localizedDescription = "Operation Timed Out"
}

func withTimeout<T>(
    _ timeout: Duration?,
    operation: @escaping () async throws -> T
) async throws -> T {
    guard let timeout = timeout else {
        return try await operation()
    }

    return try await withCheckedThrowingContinuation { continuation in
        let done = Atomic<Bool>(false)

        let task = Task {
            do {
                let v = try await operation()
                let (exchanged, _) = done.compareExchange(expected: false, desired: true)
                if exchanged {
                    continuation.resume(returning: v)
                }
            } catch {
                let (exchanged, _) = done.compareExchange(expected: false, desired: true)
                if exchanged {
                    continuation.resume(throwing: error)
                }
            }
        }

        let timeoutNs = Int(timeout / .nanoseconds(1))
        DispatchQueue.main.asyncAfter(deadline: .now().advanced(by: .nanoseconds(timeoutNs))) {
            let (exchanged, _) = done.compareExchange(expected: false, desired: true)
            if exchanged {
                task.cancel()
                continuation.resume(throwing: "Timeout elapsed")
            }
        }
    }
}

/// Atomic container until macos 15 becomes the minimum version.
class Atomic<T> {
    private var value: T
    private let lock = NSLock()

    init(_ value: T) {
        self.value = value
    }

    func load() -> T {
        self.lock.withLock {
            self.value
        }
    }

    func store(_ value: T) {
        self.lock.withLock {
            self.value = value
        }
    }
}

extension Atomic where T: Equatable {
    func compareExchange(expected: T, desired: T) -> (exchanged: Bool, original: T) {
        self.lock.withLock {
            let original = self.value
            let exchanged = self.value == expected
            if exchanged {
                self.value = desired
            }
            return (exchanged, original)
        }
    }
}
