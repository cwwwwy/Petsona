namespace Petsona.Core.Interop;

/// <summary>
/// Records the native handle and the thread that created it. The ABI is
/// thread-confined: every call, including destroy, must run on that thread.
/// </summary>
internal sealed class EngineHandle
{
    private nint _value;

    public EngineHandle(nint value)
    {
        _value = value;
        OwnerThreadId = Environment.CurrentManagedThreadId;
    }

    public int OwnerThreadId { get; }

    public nint Value => _value;

    public bool IsValid => _value != 0;

    public void AssertOwnerThread()
    {
        if (Environment.CurrentManagedThreadId != OwnerThreadId)
        {
            throw new InvalidOperationException(
                $"The Petsona engine handle is thread-confined to thread {OwnerThreadId}.");
        }
    }

    public nint Release()
    {
        var value = _value;
        _value = 0;
        return value;
    }
}
