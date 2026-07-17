# WiX MSI Installer Project

## Certificate

To create a self-signed certificate for signing the local MSIX and MSI, run the following in an admin powershell terminal.

```pwsh
cd windows/obscura-client

$cert = New-SelfSignedCertificate -Type Custom -KeyUsage DigitalSignature -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}") -Subject "CN=Sovereign Engineering Inc., O=Sovereign Engineering Inc., L=New York, S=New York, C=US, SERIALNUMBER=7746810, OID.2.5.4.15=Private Organization, OID.1.3.6.1.4.1.311.60.2.1.2=Delaware, OID.1.3.6.1.4.1.311.60.2.1.3=US" -FriendlyName "Obscura Client" -KeyExportPolicy Exportable

$bytes = $cert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx)
[System.IO.File]::WriteAllBytes("$pwd\obscura-client_TemporaryKey.pfx", $bytes)

Import-PfxCertificate -CertStoreLocation "Cert:\LocalMachine\TrustedPeople" -FilePath obscura-client_TemporaryKey.pfx
```

To test a self-signed MSI on a Windows VM, you need to install the certificate to Local Machine's Trusted People.

## Overriding MSI Version

Edit [local.props](./local.props)

```xml
<?xml version="1.0" encoding="utf-8"?>
<Project ToolsVersion="Current" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <PropertyGroup>
    <Version>0.166.31</Version>
  </PropertyGroup>
</Project>
```

## Sparse Package AppxManifest

The Sparse (external-location) identity package does the following:

- Identity
- URI protocol registration
- Notification COM registration

The real binaries (Obscura VPN.exe, obscura.exe, wintun.dll, runtime) live at the external location the MSI installs to. Obscura VPN.exe is registered via a WiX custom action. Registering grants package identity (the matching
`<msix>` element in `app.manifest` binds the EXE to it). This will be used for service's named pipe SDDL ACE.

Identity Publisher is the signing certificate subject and MUST match app.manifest's `<msix>` publisher.
Version is stamped from tag.json by build-sparse.ps1.
Referenced asset paths resolve against the external location at time of package registration.

## Signing

- **EV / KeyLocker**: when a keypair alias is set (CI). Signs with DigiCert Software
  Trust Manager's `smctl`
- **Self-signed**: when no keypair alias is set (local dev). Signs with a local `.pfx`. Override the default `../obscura-client/obscura-client_TemporaryKey.pfx` using `-SelfSignedPfx` and `-SelfSignedPfxPassword` or `$env:OBSCURA_PFX_PASSWORD`.

### GitHub Workflow

[DigiCert binary signing using GitHub Actions](https://docs.digicert.com/en/software-trust-manager/ci-cd-integrations-and-deployment-pipelines/plugins/github/binary-signing-using-github-actions.html)

| Name | Kind | Purpose |
| --- | --- | --- |
| `DIGICERT_SM_HOST` | var | KeyLocker host, e.g. `https://clientauth.one.digicert.com` |
| `DIGICERT_SM_API_KEY` | secret | Software Trust Manager API key (from portal) |
| `DIGICERT_SM_CLIENT_CERT_FILE_B64` | secret | base64 of the client-auth `.p12` (decoded to a file at runtime) |
| `DIGITCERT_SM_CLIENT_CERT_PASSWORD` | secret | password for the client-auth `.p12` |
| `DIGITCERT_SM_KEYPAIR_ALIAS` | secret | alias of the EV code-signing keypair (passed as `-p:SignKeypairAlias`) |

## Debugging an install

```powershell
msiexec /i Obscura_0.162.26_ARM64.msi /L*v install.log
```

Not a cause for concern: ExitDialog error code 2826
