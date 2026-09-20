using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Petsona.Tests;

/// <summary>
/// Loads petsona_ffi.dll once per test process from an explicit path, so the
/// tests never depend on PATH or the current working directory.
/// </summary>
internal static class FfiLibrary
{
    private const string LibraryName = "petsona_ffi";

    [ModuleInitializer]
    internal static void Initialize()
    {
        var path = Locate();
        var handle = NativeLibrary.Load(path);
        NativeLibrary.SetDllImportResolver(
            typeof(FfiLibrary).Assembly,
            (name, _, _) => name == LibraryName ? handle : nint.Zero);
    }

    private static string Locate()
    {
        var explicitPath = Environment.GetEnvironmentVariable("PETSONA_FFI_DLL");
        if (!string.IsNullOrEmpty(explicitPath) && File.Exists(explicitPath))
        {
            return explicitPath;
        }

        var candidates = new List<string>
        {
            Path.Combine(AppContext.BaseDirectory, $"{LibraryName}.dll"),
        };
        foreach (var root in EnumerateRepositoryRoots())
        {
            foreach (var profile in (string[])["release", "debug"])
            {
                candidates.Add(Path.Combine(root, "target", "x86_64-pc-windows-gnu", profile, $"{LibraryName}.dll"));
                candidates.Add(Path.Combine(root, "target", profile, $"{LibraryName}.dll"));
            }
        }

        foreach (var candidate in candidates)
        {
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        throw new FileNotFoundException(
            $"Cannot find {LibraryName}.dll. Build it with " +
            "'cargo build -p petsona-ffi --release --target x86_64-pc-windows-gnu' " +
            "or set PETSONA_FFI_DLL. Searched: " + string.Join(", ", candidates));
    }

    private static IEnumerable<string> EnumerateRepositoryRoots()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                yield return directory.FullName;
            }
        }
    }
}
