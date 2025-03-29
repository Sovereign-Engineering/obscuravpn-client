import Testing

@Test(.timeLimit(.minutes(1)))
func testAsyncMutex() async throws {
    let mutex = AsyncMutex(false)

    await withTaskGroup(of: Void.self) { tasks in
        for _ in 0 ..< 100 {
            tasks.addTask {
                await mutex.withLock { mutex_guard in
                    #expect(!mutex_guard.value)
                    mutex_guard.value = true
                    #expect(mutex_guard.value)
                    try! await Task.sleep(seconds: 0.01)
                    #expect(mutex_guard.value)
                    mutex_guard.value = false
                }
            }
        }
        await tasks.waitForAll()
    }

    // Write your test here and use APIs like `#expect(...)` to check expected conditions.
    #expect(true)
}
