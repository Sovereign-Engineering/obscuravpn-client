package net.obscura.vpnclientapp.client

data class JsonFfiException(
    val data: String?,
) : Exception("JSON FFI Exception with Data $data")
