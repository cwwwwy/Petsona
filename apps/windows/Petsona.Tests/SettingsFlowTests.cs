using System.Text.Json;
using Petsona.Core;
using Petsona.Core.Interop;
using Xunit;

namespace Petsona.Tests;

/// <summary>
/// Mirrors the macOS settings round trip: DeepSeek config, memory config,
/// persona creation and a remembered fact all land in the worker projection.
/// </summary>
public sealed class SettingsFlowTests
{
    [Fact]
    public void SettingsMemoryAndPersonaCommandsRoundTrip()
    {
        using var home = new IsolatedHome();
        using var client = new EngineClient(home.Path);
        WaitUntilReady(client);

        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.UpdateDeepSeekConfig, text: Json(new Dictionary<string, object?>
            {
                ["baseUrl"] = "https://example.invalid/v1",
                ["model"] = "test-model",
                ["apiKeyEnv"] = "TEST_DEEPSEEK_KEY",
                ["timeoutSeconds"] = 9,
                ["maxTokens"] = 64,
                ["temperature"] = 0.4,
                ["thinkingDisabled"] = true,
            })));
        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.UpdateMemoryConfig, text: Json(new Dictionary<string, object?>
            {
                ["enabled"] = true,
                ["recentEvents"] = 7,
                ["factLimit"] = 12,
            })));
        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.UpdateGreetingConfig, text: Json(new Dictionary<string, object?>
            {
                ["enabled"] = true,
                ["idleMinutes"] = 45,
                ["cooldownMinutes"] = 90,
                ["maxChars"] = 24,
            })));

        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.CreatePersona, text: Json(new Dictionary<string, object?>
            {
                ["id"] = "native-test",
                ["name"] = "原生测试",
            })));

        Assert.True(
            WaitFor(client, () => client.Text(PetsonaTextField.PersonaId) == "native-test"),
            "persona selection did not converge");

        Assert.Equal(
            PetsonaStatus.Ok,
            client.Send(PetsonaCommandKind.RememberFact, text: Json(new Dictionary<string, object?>
            {
                ["key"] = "喜欢",
                ["value"] = "安静音乐",
            })));

        Assert.True(
            WaitFor(client, () => client.Text(PetsonaTextField.Memory).Contains("安静音乐")),
            "memory fact did not appear");

        Assert.Contains("原生测试", client.Text(PetsonaTextField.Personas));
        Assert.Contains("test-model", client.Text(PetsonaTextField.DeepSeekConfig));
        Assert.True(
            WaitFor(client, () => client.Text(PetsonaTextField.Memory).Contains("\"idleMinutes\":45")),
            "greeting config did not land in the projection");
        Assert.Contains("安静音乐", client.Text(PetsonaTextField.Memory));
    }

    private static string Json(Dictionary<string, object?> payload)
    {
        return JsonSerializer.Serialize(payload);
    }

    private static void WaitUntilReady(EngineClient client)
    {
        Assert.True(WaitFor(client, () => client.Snapshot().Ready != 0), "runtime did not become ready");
    }

    private static bool WaitFor(EngineClient client, Func<bool> condition)
    {
        for (var attempt = 0; attempt < 200; attempt++)
        {
            client.Tick();
            if (condition())
            {
                return true;
            }

            Thread.Sleep(10);
        }

        return false;
    }
}
