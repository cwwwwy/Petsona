using Petsona.Core;
using Petsona.Core.Interop;
using Xunit;

namespace Petsona.Tests;

/// <summary>
/// Real cross-language round trips through the Rust runtime worker. Every
/// test creates an isolated home before the engine exists.
/// </summary>
public sealed class EngineClientTests
{
    [Fact]
    public void EmptyHomeBecomesReadyWithoutAPet()
    {
        using var home = new IsolatedHome();
        using var client = new EngineClient(home.Path);
        var snapshot = WaitUntilReady(client);

        Assert.Equal(NativeMethods.AbiVersion, snapshot.AbiVersion);
        Assert.NotEqual(0, snapshot.Ready);
        Assert.Equal(0, snapshot.HasPet);
        Assert.Equal(0, snapshot.Faulted);
        Assert.True(File.Exists(System.IO.Path.Combine(home.Path, "config.json")));
    }

    [Fact]
    public void CommandsRoundTripThroughTheWorkerProjection()
    {
        using var home = new IsolatedHome();
        using var client = new EngineClient(home.Path);
        WaitUntilReady(client);

        Assert.Equal(PetsonaStatus.Ok, client.Send(PetsonaCommandKind.SetScale, value: 1.5));
        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.ShowBubble, ttlMilliseconds: 5_000, text: "隔离测试"));

        Assert.Equal("隔离测试", WaitForText(client, PetsonaTextField.Bubble, "隔离测试"));
        Assert.Equal(1.5f, client.Snapshot().Scale, 3);

        Assert.Equal(PetsonaStatus.Ok, client.Send(PetsonaCommandKind.ClearBubble));
        Assert.Equal(string.Empty, WaitForText(client, PetsonaTextField.Bubble, string.Empty));
    }

    [Fact]
    public void InvalidCommandIsRejectedAndReportsLastError()
    {
        using var home = new IsolatedHome();
        using var client = new EngineClient(home.Path);
        WaitUntilReady(client);

        Assert.Equal(PetsonaStatus.InvalidArgument, client.SendRaw(999));
        Assert.False(string.IsNullOrWhiteSpace(EngineClient.LastError()));
    }

    private static PetsonaSnapshot WaitUntilReady(EngineClient client)
    {
        for (var attempt = 0; attempt < 200; attempt++)
        {
            client.Tick();
            var snapshot = client.Snapshot();
            if (snapshot.Ready != 0)
            {
                return snapshot;
            }

            Thread.Sleep(10);
        }

        throw new Xunit.Sdk.XunitException("The isolated runtime did not become ready.");
    }

    private static string WaitForText(EngineClient client, PetsonaTextField field, string expected)
    {
        var current = string.Empty;
        for (var attempt = 0; attempt < 100; attempt++)
        {
            client.Tick();
            current = client.Text(field);
            if (current == expected)
            {
                return current;
            }

            Thread.Sleep(10);
        }

        return current;
    }
}
