namespace Petsona.Tests;

/// <summary>Per-test data directory with the state server disabled.</summary>
internal sealed class IsolatedHome : IDisposable
{
    public IsolatedHome()
        : this(stateServerPort: null)
    {
    }

    public IsolatedHome(int? stateServerPort)
    {
        Path = System.IO.Path.Combine(
            System.IO.Path.GetTempPath(), $"petsona-native-test-{Guid.NewGuid():N}");
        Directory.CreateDirectory(Path);
        var config = stateServerPort is int port
            ? $"{{\"stateServer\":{{\"enabled\":true,\"port\":{port}}}}}"
            : "{\"stateServer\":{\"enabled\":false}}";
        File.WriteAllText(System.IO.Path.Combine(Path, "config.json"), config);
    }

    public string Path { get; }

    public void Dispose()
    {
        try
        {
            Directory.Delete(Path, recursive: true);
        }
        catch (IOException)
        {
            // The runtime worker may still be releasing its lock file.
        }
    }
}
