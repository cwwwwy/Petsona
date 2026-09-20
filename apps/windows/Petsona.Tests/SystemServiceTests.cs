using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class SystemServiceTests
{
    [Fact]
    public void AutostartRoundTripsThroughAnIsolatedValueName()
    {
        var valueName = $"Petsona-test-{Guid.NewGuid():N}";
        Environment.SetEnvironmentVariable("PETSONA_AUTOSTART_VALUE_NAME", valueName);
        try
        {
            SystemServices.SetAutostart(false);
            Assert.False(SystemServices.IsAutostartEnabled());

            SystemServices.SetAutostart(true);
            Assert.True(SystemServices.IsAutostartEnabled());

            SystemServices.SetAutostart(false);
            Assert.False(SystemServices.IsAutostartEnabled());
        }
        finally
        {
            SystemServices.SetAutostart(false);
            Environment.SetEnvironmentVariable("PETSONA_AUTOSTART_VALUE_NAME", null);
        }
    }
}
