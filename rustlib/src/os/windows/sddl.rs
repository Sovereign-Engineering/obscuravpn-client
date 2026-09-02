//! Builds Windows security descriptors and attaches them to objects.
//!
//! This module provides a type-safe builder for constructing access control rules, rendered as SDDL
//! (Security Descriptor Definition Language) strings that the Windows kernel parses into security
//! descriptors.
//!
//! Every raw pointer this crate hands to the Win32 security APIs lives here, so callers stay safe.

use camino::Utf8Path;
use std::ffi::c_void;
use std::fmt;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::CreateDirectoryW;
use windows::core::PCWSTR;

use crate::int_helper::try_usize_into_u32;

const SDDL_REVISION_1: u32 = 1;
/// Value for `SECURITY_ATTRIBUTES.nLength`.
pub const SA_LENGTH: u32 = try_usize_into_u32(std::mem::size_of::<SECURITY_ATTRIBUTES>()).expect("SECURITY_ATTRIBUTES size fits into u32");

#[derive(Clone, Copy)]
pub enum FileRights {
    /// `FA` -- full access.
    FullAccess,
    /// `FRFW` -- file generic read + file generic write.
    ReadWrite,
    /// `FR` -- file generic read.
    Read,
    /// `FRFX` -- file generic read + execute, which on a dir means listing it and traversing into it.
    ReadTraverse,
}

impl FileRights {
    fn as_sddl(self) -> &'static str {
        match self {
            FileRights::FullAccess => "FA",
            FileRights::ReadWrite => "FRFW",
            FileRights::Read => "FR",
            FileRights::ReadTraverse => "FRFX",
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

    /// `BU` -- the `BUILTIN\Users` group, i.e. every local user, including the one running the GUI.
    pub const fn builtin_users() -> Self {
        Self("BU")
    }

    /// `IU` -- `NT AUTHORITY\INTERACTIVE`, i.e. users logged on interactively (console or RDP),
    /// excluding network logons and service accounts.
    pub const fn interactive() -> Self {
        Self("IU")
    }

    /// `OW` -- OWNER RIGHTS, which resolves to whoever owns the object, so the creator keeps access
    /// without naming a specific account.
    pub const fn owner_rights() -> Self {
        Self("OW")
    }

    fn as_sddl(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy)]
pub enum Inherit {
    /// No inherit flags: the ACE applies to this object alone.
    None,
    /// `CI` -- child dirs, at any depth.
    Dirs,
    /// `OI` -- child files, at any depth.
    Files,
    /// `OICI` -- both.
    DirsAndFiles,
}

impl Inherit {
    fn as_sddl(self) -> &'static str {
        match self {
            Inherit::None => "",
            Inherit::Dirs => "CI",
            Inherit::Files => "OI",
            Inherit::DirsAndFiles => "OICI",
        }
    }
}

enum Ace {
    /// Plain `(A;<flags>;<rights>;;;<trustee>)`.
    Allow {
        rights: FileRights,
        trustee: Trustee,
        inherit: Inherit,
    },
    /// Conditional `(XA;;<rights>;;;WD;(WIN://SYSAPPID Contains "<pfn>"))`.
    PackagedAllow { rights: FileRights, pfn: &'static str },
}

/// Discretionary Access Control List Builder
#[derive(Default)]
pub struct DACL {
    aces: Vec<Ace>,
}

impl DACL {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plain `(A;<inherit>;<rights>;;;<trustee>)` ACE.
    pub fn allow(mut self, rights: FileRights, trustee: Trustee, inherit: Inherit) -> Self {
        self.aces.push(Ace::Allow { rights, trustee, inherit });
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
// https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-strings
impl fmt::Display for DACL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `D:P` drops whatever the object would inherit from its parent.
        f.write_str("D:P")?;
        for ace in &self.aces {
            match ace {
                Ace::Allow { rights, trustee, inherit } => write!(f, "(A;{};{};;;{})", inherit.as_sddl(), rights.as_sddl(), trustee.as_sddl())?,
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
        let wide = wide_nul(sddl);
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

    /// Creates `path` as a new dir carrying this descriptor, so it never exists with the permissions
    /// it would have inherited from its parent.
    pub fn create_dir(&self, path: &Utf8Path) -> std::io::Result<()> {
        let path_wide = wide_nul(path.as_str());
        let attributes = SECURITY_ATTRIBUTES { nLength: SA_LENGTH, lpSecurityDescriptor: self.psd.0, bInheritHandle: false.into() };
        // SAFETY: `path_wide` is NUL-terminated and outlives the call, and `attributes` borrows a
        // descriptor owned by `self` for the same span.
        unsafe { CreateDirectoryW(PCWSTR(path_wide.as_ptr()), Some(&attributes)) }.map_err(std::io::Error::other)
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

fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PFN: &str = env!("OBSCURA_PACKAGE_FAMILY_NAME");

    /// Exact shape the service's pipe uses: LocalSystem/Admins full access + a conditional ACE
    /// pinning read/write to our packaged GUI.
    #[test]
    fn pipe_sddl_renders() {
        let s = DACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system(), Inherit::None)
            .allow(FileRights::FullAccess, Trustee::builtin_administrators(), Inherit::None)
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
        DACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system(), Inherit::None)
            .allow(FileRights::FullAccess, Trustee::builtin_administrators(), Inherit::None)
            .allow_packaged(FileRights::ReadWrite, PFN)
            .build()
            .expect("kernel should accept the conditional SYSAPPID SDDL");
    }

    /// Debug pipe shape: interactively logged-on users instead of the packaged-GUI pin.
    #[test]
    fn debug_pipe_sddl_round_trips_through_kernel() {
        DACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system(), Inherit::None)
            .allow(FileRights::FullAccess, Trustee::builtin_administrators(), Inherit::None)
            .allow(FileRights::ReadWrite, Trustee::interactive(), Inherit::None)
            .build()
            .expect("kernel should accept the interactive-users SDDL");
    }

    /// Inheritance is per-ACE, which is what lets one call on a dir give its child dirs and child
    /// files different rights.
    #[test]
    fn inherit_flags_render_per_ace() {
        DACL::new()
            .allow(FileRights::FullAccess, Trustee::owner_rights(), Inherit::DirsAndFiles)
            .allow(FileRights::ReadTraverse, Trustee::builtin_users(), Inherit::Dirs)
            .allow(FileRights::Read, Trustee::builtin_users(), Inherit::Files)
            .build()
            .expect("kernel should accept inheritable aces");
    }
}
