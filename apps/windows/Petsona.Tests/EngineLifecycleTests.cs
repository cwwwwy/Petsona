using System.Net;
using System.Net.Sockets;
using Petsona.Core;
using Petsona.Core.Interop;
using Xunit;

namespace Petsona.Tests;

/// <summary>
/// Fault and shutdown rules: a disposed or misused client never reaches the
/// ABI, destroying the engine releases the state-server port and the
/// instance lock, and the saved position survives a restart.
/// </summary>
public sealed class EngineLifecycleTests
{
    [Fact]
    public void DisposedClientRejectsFurtherCalls()
    {
        using var home = new IsolatedHome();
        var client = new EngineClient(home.Path);
        WaitUntilReady(client);
        client.Dispose();

        Assert.Throws<ObjectDisposedException>(() => client.Tick());
        Assert.Throws<ObjectDisposedException>(() => client.Snapshot());
        Assert.Throws<ObjectDisposedException>(() => client.Text(PetsonaTextField.State));
        Assert.Throws<ObjectDisposedException>(() => client.Send(PetsonaCommandKind.SetScale, value: 1.0));

        client.Dispose(); // must be idempotent
    }

    [Fact]
    public void CrossThreadCallsAreRejected()
    {
        using var home = new IsolatedHome();
        using var client = new EngineClient(home.Path);
        WaitUntilReady(client);

        var error = Assert.Throws<InvalidOperationException>(
            () => Task.Run(() => client.Tick()).GetAwaiter().GetResult());
        Assert.Contains("thread", error.Message, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void DisposeReleasesTheStateServerPortAndTheInstanceLock()
    {
        var port = FreeLoopbackPort();
        using var home = new IsolatedHome(port);
        var client = new EngineClient(home.Path);
        WaitUntilReady(client);
        Assert.True(WaitFor(() => CanConnect(port)), "the state server should accept connections");

        client.Dispose();

        Assert.True(WaitFor(() => CanBind(port)), "the port must be free after dispose");
        using var second = new EngineClient(home.Path);
        WaitUntilReady(second);
    }

    [Fact]
    public void SecondEngineOnTheSameHomeFaultsInsteadOfSilentlyFailing()
    {
        using var home = new IsolatedHome();
        using var first = new EngineClient(home.Path);
        WaitUntilReady(first);

        using var second = new EngineClient(home.Path);
        var faultedOrRejected = false;
        for (var attempt = 0; attempt < 200 && !faultedOrRejected; attempt++)
        {
            try
            {
                second.Tick();
                faultedOrRejected = second.Snapshot().Faulted != 0;
            }
            catch (EngineException error) when (
                error.Status is PetsonaStatus.RuntimeFailed or PetsonaStatus.Panic)
            {
                faultedOrRejected = true;
            }

            if (!faultedOrRejected)
            {
                Thread.Sleep(10);
            }
        }

        Assert.True(faultedOrRejected, "the second engine should become terminal");
        Assert.False(string.IsNullOrWhiteSpace(second.Text(PetsonaTextField.Error)));
    }

    [Fact]
    public void SavedPositionSurvivesAnEngineRestart()
    {
        using var home = new IsolatedHome();
        using (var client = new EngineClient(home.Path))
        {
            WaitUntilReady(client);
            Assert.Equal(PetsonaStatus.Ok, client.Send(PetsonaCommandKind.SetPosition, text: "640,480"));
            Assert.True(
                WaitFor(() => client.Text(PetsonaTextField.Position) == "640,480"),
                "position projection did not converge");
        }

        using var restarted = new EngineClient(home.Path);
        WaitUntilReady(restarted);
        Assert.Equal("640,480", restarted.Text(PetsonaTextField.Position));
    }

    private static int FreeLoopbackPort()
    {
        var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;
        listener.Stop();
        return port;
    }

    private static bool CanConnect(int port)
    {
        try
        {
            using var client = new TcpClient();
            return client.ConnectAsync(IPAddress.Loopback, port).Wait(TimeSpan.FromMilliseconds(500))
                && client.Connected;
        }
        catch (Exception exception) when (exception is SocketException or AggregateException)
        {
            return false;
        }
    }

    private static bool CanBind(int port)
    {
        try
        {
            var listener = new TcpListener(IPAddress.Loopback, port);
            listener.Start();
            listener.Stop();
            return true;
        }
        catch (SocketException)
        {
            return false;
        }
    }

    private static void WaitUntilReady(EngineClient client)
    {
        Assert.True(WaitFor(() => client.Snapshot().Ready != 0), "runtime did not become ready");
    }

    private static bool WaitFor(Func<bool> condition)
    {
        for (var attempt = 0; attempt < 200; attempt++)
        {
            if (condition())
            {
                return true;
            }

            Thread.Sleep(10);
        }

        return false;
    }
}
