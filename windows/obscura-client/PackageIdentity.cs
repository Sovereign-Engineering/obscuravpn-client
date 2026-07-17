using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

namespace Obscura_Client;

internal static class PackageIdentity
{
    private const uint APPMODEL_ERROR_NO_PACKAGE = 15700;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern uint GetCurrentPackageFullName(
        ref int packageFullNameLength,
        StringBuilder? packageFullName);

    public static bool IsPackagedProcess()
    {
        int n = 0;
        uint rc = GetCurrentPackageFullName(ref n, null);
        // When unpackaged, GetCurrentPackageFullName always returns APPMODEL_ERROR_NO_PACKAGE
        return rc != APPMODEL_ERROR_NO_PACKAGE;
    }
}
