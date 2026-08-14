//! Windows permissions for a service debug bundle: readable by every local user so the
//! unprivileged client can collect it, writable by nobody but the service.

use crate::os::windows::sddl::{DACL, FileRights, Inherit, SecurityDescriptor, Trustee};
use camino::Utf8Path;

/// Full access for the dir's owner and for the service, so it can populate and later delete the
/// bundle, plus read for local users: dirs get read + list + traverse, files get plain read. Every
/// ACE is inheritable, so entries written into the bundle come out with these rights already.
fn users_readable_dir_dacl() -> DACL {
    DACL::new()
        .allow(FileRights::FullAccess, Trustee::owner_rights(), Inherit::DirsAndFiles)
        .allow(FileRights::FullAccess, Trustee::local_system(), Inherit::DirsAndFiles)
        .allow(FileRights::FullAccess, Trustee::builtin_administrators(), Inherit::DirsAndFiles)
        .allow(FileRights::ReadTraverse, Trustee::builtin_users(), Inherit::Dirs)
        .allow(FileRights::Read, Trustee::builtin_users(), Inherit::Files)
}

pub(crate) fn create_users_readable_dir(path: &Utf8Path) -> std::io::Result<()> {
    build_descriptor(users_readable_dir_dacl())?.create_dir(path)
}

fn build_descriptor(dacl: DACL) -> std::io::Result<SecurityDescriptor> {
    dacl.build()
        .inspect_err(|error| tracing::error!(message_id = "xB8kQn4T", ?error, sddl =% dacl, "failed to build security descriptor"))
}
