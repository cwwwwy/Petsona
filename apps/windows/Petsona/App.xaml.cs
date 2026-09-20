using Microsoft.UI.Xaml;

namespace Petsona;

/// <summary>WinUI entry point that hosts the B1 controller.</summary>
public partial class App : Application
{
    private AppController? _controller;

    // WinUI exits when its last Window closes. The pet window is a native
    // Win32 window, so without this hidden anchor, closing Settings or the
    // Composer would terminate the whole app.
    private Window? _lifetimeAnchor;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            _lifetimeAnchor = new Window { Title = "Petsona" };
            _controller = new AppController(Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
            _controller.Start();
        }
        catch (Exception exception)
        {
            AppController.TryWriteErrorLog("startup", exception);
            throw;
        }
    }
}
