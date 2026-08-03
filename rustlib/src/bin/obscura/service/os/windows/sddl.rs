//! Builds Windows security descriptors for the service's named pipe.
//!
//! This module provides a type-safe builder for constructing access control rules. The pipe
//! allows LocalSystem and Administrators full access, and uses a package-identity check to
//! permit only our GUI (deployed as a Sparse Package MSIX) read/write access. The builder
//! generates SDDL (Security Descriptor Definition Language) strings that the Windows kernel
//! parses into security descriptors.

use std::ffi::c_void;
use std::fmt;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::PSECURITY_DESCRIPTOR;
use windows::core::PCWSTR;

const SDDL_REVISION_1: u32 = 1;

#[derive(Clone, Copy)]
pub enum FileRights {
    /// `FA` -- full access.
    FullAccess,
    /// `FRFW` -- file generic read + file generic write.
    ReadWrite,
}

impl FileRights {
    fn as_sddl(self) -> &'static str {
        match self {
            FileRights::FullAccess => "FA",
            FileRights::ReadWrite => "FRFW",
        }
    }
}

/// A two-letter SDDL trustee alias.
#[derive(Clone, Copy)]
pub struct Trustee(&'static str);

impl Trustee {
    /// `SY` -- the LocalSystem account.
    pub const fn local_system() -> Self {
        Self("SY")
    }

    /// `BA` -- the `BUILTIN\Administrators` group.
    pub const fn builtin_administrators() -> Self {
        Self("BA")
    }

    fn as_sddl(self) -> &'static str {
        self.0
    }
}

enum Ace {
    /// Plain `(A;;<rights>;;;<trustee>)`.
    Allow { rights: FileRights, trustee: Trustee },
    /// Conditional `(XA;;<rights>;;;WD;(WIN://SYSAPPID Contains "<pfn>"))`.
    PackagedAllow { rights: FileRights, pfn: &'static str },
}

#[derive(Default)]
pub struct PipeDACL {
    aces: Vec<Ace>,
}

impl PipeDACL {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plain `(A;;<rights>;;;<trustee>)` ACE.
    pub fn allow(mut self, rights: FileRights, trustee: Trustee) -> Self {
        self.aces.push(Ace::Allow { rights, trustee });
        self
    }

    pub fn allow_packaged(mut self, rights: FileRights, pfn: &'static str) -> Self {
        debug_assert!(
            pfn.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_'),
            "package family name `{pfn}` has characters unsafe to embed in an SDDL conditional"
        );
        self.aces.push(Ace::PackagedAllow { rights, pfn });
        self
    }

    /// Parse the rendered SDDL into a [`SecurityDescriptor`]. The SDDL is syntactically valid by
    /// construction, so any error is a kernel-level rejection or out-of-memory condition.
    pub fn build(&self) -> std::io::Result<SecurityDescriptor> {
        SecurityDescriptor::from_sddl(&self.to_string())
    }
}

// https://github.com/microsoft/WindowsAppSDK/discussions/3348#discussioncomment-8781167
// https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-strings?source=recommendations
impl fmt::Display for PipeDACL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("D:P")?;
        for ace in &self.aces {
            match ace {
                Ace::Allow { rights, trustee } => write!(f, "(A;;{};;;{})", rights.as_sddl(), trustee.as_sddl())?,
                Ace::PackagedAllow { rights, pfn } => write!(f, "(XA;;{};;;WD;(WIN://SYSAPPID Contains \"{pfn}\"))", rights.as_sddl())?,
            }
        }
        Ok(())
    }
}

/// Owns a self-relative `SECURITY_DESCRIPTOR` allocated by
/// `ConvertStringSecurityDescriptorToSecurityDescriptorW`; frees it with `LocalFree` on drop.
pub struct SecurityDescriptor {
    psd: PSECURITY_DESCRIPTOR,
}

impl SecurityDescriptor {
    fn from_sddl(sddl: &str) -> std::io::Result<Self> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut psd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string valid for the call. On success Windows
        // allocates a descriptor we release with `LocalFree` in `Drop`.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(wide.as_ptr()), SDDL_REVISION_1, &mut psd, None)?;
        }
        Ok(Self { psd })
    }

    /// Pointer to the descriptor, for `SECURITY_ATTRIBUTES.lpSecurityDescriptor`.
    pub fn as_ptr(&self) -> *mut c_void {
        self.psd.0
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: `psd` was allocated by `ConvertStringSecurityDescriptorToSecurityDescriptorW`
        // (LocalAlloc-backed) and has not been freed yet.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.psd.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PFN: &str = env!("OBSCURA_PACKAGE_FAMILY_NAME");

    /// Exact shape the service's pipe uses: LocalSystem/Admins full access + a conditional ACE
    /// pinning read/write to our packaged GUI.
    #[test]
    fn pipe_sddl_renders() {
        let s = PipeDACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system())
            .allow(FileRights::FullAccess, Trustee::builtin_administrators())
            .allow_packaged(FileRights::ReadWrite, PFN)
            .to_string();
        assert_eq!(
            s,
            format!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(XA;;FRFW;;;WD;(WIN://SYSAPPID Contains \"{PFN}\"))")
        );
    }

    /// The kernel's SDDL parser must accept every shape this builder emits, including the
    /// conditional `WIN://SYSAPPID` ACE.
    #[test]
    fn pipe_sddl_round_trips_through_kernel() {
        PipeDACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system())
            .allow(FileRights::FullAccess, Trustee::builtin_administrators())
            .allow_packaged(FileRights::ReadWrite, PFN)
            .build()
            .expect("kernel should accept the conditional SYSAPPID SDDL");
    }
}
