import Foundation

class WatchableValue<T> {
    private var lock: NSLock = .init()
    private var value: T
    private var continuations: [CheckedContinuation<T, Never>] = []

    init(_ value: T) {
        self.value = value
    }

    func publish(_ value: T) {
        _ = self.update { current in
            current = value
        }
    }

    func update(_ f: (inout T) -> Void) -> T {
        self.lock.withLock {
            f(&self.value)
            for continuation in self.continuations {
                continuation.resume(returning: self.value)
            }
            self.continuations.removeAll()
            return self.value
        }
    }

    /// Get the current value.
    func get() -> T? {
        self.lock.withLock {
            self.value
        }
    }

    /// Get the current value if `predicate` returns true, otherwise return the next published value
    func getIfOrNext(_ predicate: (T) -> Bool) async -> T {
        await withCheckedContinuation { continuation in
            self.lock.withLock {
                if predicate(self.value) {
                    continuation.resume(returning: self.value)
                } else {
                    self.continuations.append(continuation)
                }
            }
        }
    }
}
