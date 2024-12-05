import Foundation
import NetworkExtension

// TODO: Use `std::panic::set_backtrace_style()` in Rust initialization once stabilized.
// https://doc.rust-lang.org/std/panic/fn.set_backtrace_style.html
setenv("RUST_BACKTRACE", "1", 1)

autoreleasepool {
    NEProvider.startSystemExtensionMode()
}

dispatchMain()
