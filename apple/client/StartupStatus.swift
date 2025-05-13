enum StartupStatus {
    case initial
    #if os(macOS)
        case networkExtensionInit(NetworkExtensionInit, NetworkExtensionInitStatus)
    #endif
    case tunnelProviderInit(TunnelProviderInit, TunnelProviderInitStatus)
    #if os(macOS)
        case askToRegisterLoginItem(ObservableValue<Bool>)
    #endif
    case ready
}
