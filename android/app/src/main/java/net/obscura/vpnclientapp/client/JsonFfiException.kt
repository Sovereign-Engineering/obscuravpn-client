package net.obscura.vpnclientapp.client

import androidx.annotation.Keep

// Instances of this class are only constructed from the Rust side, so without this annotation
// release builds would strip out the class definition.
@Keep
data class JsonFfiException(
    val data: String?,
) : Exception("JSON FFI exception: $data")
