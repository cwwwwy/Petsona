using Microsoft.Win32;

namespace Petsona.Core;

/// <summary>
/// Windows shell integration for the native frontend. Autostart lives in
/// <c>HKCU\Software\Microsoft\Windows\CurrentVersion\Run</c>; tests isolate
/// the value name through <c>PETSONA_AUTOSTART_VALUE_NAME</c>.
/// </summary>
public static class SystemServices
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string DefaultValueName = "Petsona";

    public static string AutostartValueName =>
        Environment.GetEnvironmentVariable("PETSONA_AUTOSTART_VALUE_NAME") is { Length: > 0 } custom
            ? custom
            : DefaultValueName;

    public static bool IsAutostartEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: false);
        return key?.GetValue(AutostartValueName) is string value && value.Length > 0;
    }

    public static void SetAutostart(bool enabled)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKeyPath, writable: true)
            ?? throw new InvalidOperationException("Cannot open the HKCU Run key.");
        if (enabled)
        {
            var executable = Environment.ProcessPath
                ?? throw new InvalidOperationException("Cannot resolve the Petsona executable path.");
            key.SetValue(AutostartValueName, $"\"{executable}\"", RegistryValueKind.String);
        }
        else
        {
            key.DeleteValue(AutostartValueName, throwOnMissingValue: false);
        }
    }
}
