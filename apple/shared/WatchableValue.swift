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
    func get() -> T {
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

    /// Returns the current value if `predicate` returns true, otherwise returns the next published value that does
    func waitUntil(_ predicate: (T) -> Bool) async -> T {
        while true {
            let value = await self.getIfOrNext(predicate)
            if predicate(value) {
                return value
            }
        }
    }

    func waitUntilWithTimeout(_ timeout: Duration, _ predicate: @escaping (T) -> Bool) async -> T? {
        do {
            return try await withTimeout(timeout, operation: { await self.waitUntil(predicate) })
        } catch {
            return nil
        }
    }
}
