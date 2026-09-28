use std::mem::ManuallyDrop;
use tokio::task::spawn_blocking;
use windows::Win32::Foundation::{E_OUTOFMEMORY, VARIANT_TRUE};
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, NET_FW_ACTION_ALLOW, NET_FW_PROFILE2_ALL, NET_FW_RULE_DIR_OUT, NetFwPolicy2, NetFwRule,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_ARRAY, VT_VARIANT};
use windows::core::BSTR;

const RULE_NAME: &str = "Obscura VPN Tunnel";
const RULE_GROUPING: &str = "Obscura VPN";
const RULE_DESCRIPTION: &str = "Allow outbound traffic through the Obscura VPN tunnel adapter";

/// The tunnel adapter is classified as a Public network. Explicitly allow outbound traffic on it,
/// so a default-block outbound policy on the Public profile doesn't stop traffic before it reaches the tunnel.
/// Block rules still take precedence over this rule.
pub async fn allow_tunnel_outbound(adapter: &wintun::Adapter) -> Result<(), ()> {
    let interface = adapter
        .get_name()
        .map_err(|error| tracing::error!(message_id = "C2hcq90M", ?error, "failed to get adapter name for firewall rule"))?;
    spawn_blocking(move || {
        // SAFETY: Only called by `with_mta` below, which keeps COM initialized for the whole call.
        let replace = || unsafe { replace_rule(&interface) };
        // SAFETY: `replace` returns `()`, so no COM objects outlive the call.
        unsafe { with_mta(replace) }
    })
    .await
    .map_err(|error| tracing::error!(message_id = "fW3kQp8x", ?error, "failed to join firewall rule task"))?
    .map_err(|error| tracing::error!(message_id = "Lr5vNc2d", ?error, "failed to add tunnel firewall rule"))?;
    tracing::info!(message_id = "Ht9mYs4b", "added tunnel firewall rule");
    Ok(())
}

/// ## Safety
/// `f` must not let COM objects outlive the call, since they would be used after `CoUninitialize`.
unsafe fn with_mta<T>(f: impl FnOnce() -> windows::core::Result<T>) -> windows::core::Result<T> {
    // SAFETY: No preconditions. Only a successful initialization is balanced by `CoUninitialize` below.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    let result = f();
    // SAFETY: Balances the successful `CoInitializeEx` above, and the caller guarantees no COM objects from `f` are alive.
    unsafe { CoUninitialize() };
    result
}

/// Builds an outbound allow rule scoped to `interface`, the tunnel adapter's friendly name, and swaps it in for any existing rule named
/// `RULE_NAME`, so repeated calls leave exactly one up-to-date rule.
///
/// ## Safety
/// COM must be initialized on the calling thread for the duration of the call.
unsafe fn replace_rule(interface: &str) -> windows::core::Result<()> {
    // SAFETY: COM is initialized per this function's contract.
    let rule: INetFwRule = unsafe { CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER) }?;
    let name = BSTR::from(RULE_NAME);
    // SAFETY: `rule` is a valid interface, and the arguments are valid BSTRs and enum values.
    unsafe {
        rule.SetName(&name)?;
        rule.SetGrouping(&BSTR::from(RULE_GROUPING))?;
        rule.SetDescription(&BSTR::from(RULE_DESCRIPTION))?;
        rule.SetDirection(NET_FW_RULE_DIR_OUT)?;
        rule.SetAction(NET_FW_ACTION_ALLOW)?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0)?;
        rule.SetEnabled(VARIANT_TRUE)?;
    }
    let interfaces = string_array_variant(interface)?;
    // SAFETY: `rule` is a valid interface and `interfaces` is a well-formed VARIANT, which the callee copies.
    unsafe { rule.SetInterfaces(&interfaces) }?;

    // SAFETY: COM is initialized per this function's contract.
    let policy: INetFwPolicy2 = unsafe { CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER) }?;
    // SAFETY: `policy`, its rule collection and `rule` are valid interfaces, and `name` is a valid BSTR.
    unsafe {
        let rules = policy.Rules()?;
        rules.Remove(&name)?;
        rules.Add(&rule)
    }
}

/// `INetFwRule::Interfaces` takes a `VT_ARRAY | VT_VARIANT` of `VT_BSTR`, which the crate has no conversion for.
fn string_array_variant(value: &str) -> windows::core::Result<VARIANT> {
    // SAFETY: No preconditions. Returns null on failure, which is checked below.
    let array = unsafe { SafeArrayCreateVector(VT_VARIANT, 0, 1) };
    if array.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    // Owns `array` from here on: `VARIANT`'s `Drop` calls `VariantClear`, which destroys the array, including on the error path below.
    let variant = VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_ARRAY | VT_VARIANT,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { parray: array },
            }),
        },
    };
    let element = VARIANT::from(value);
    // SAFETY: `array` is a valid one-element vector, index 0 is in bounds, and `element` is a well-formed VARIANT, which is copied into the array.
    unsafe { SafeArrayPutElement(array, &0, (&raw const element).cast()) }?;
    Ok(variant)
}
