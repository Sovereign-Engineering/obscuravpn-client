import Foundation

class ObservableValue<T> {
    var lock: NSLock = .init()
    var set = false
    var value: T?
    var continuations: [CheckedContinuation<T, Never>] = []

    func publish(_ value: T) {
        self.lock.withLock {
            self.set = true
            self.value = value
            for continuation in self.continuations {
                continuation.resume(returning: value)
            }
            self.continuations.removeAll()
        }
    }

    /// Get the value.
    ///
    /// This will block if the value hasn't been set yet.
    func get() async -> T {
        await withCheckedContinuation { continuation in
            self.lock.withLock {
                if self.set {
                    continuation.resume(returning: self.value!)
                } else {
                    self.continuations.append(continuation)
                }
            }
        }
    }

    /// Get the value if it has been set.
    func tryGet() -> T? {
        self.lock.withLock {
            self.value
        }
    }
}
