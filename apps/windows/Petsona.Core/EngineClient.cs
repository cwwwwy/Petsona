using System.Text;
using Petsona.Core.Interop;

namespace Petsona.Core;

/// <summary>Raised when an ABI call fails; the message carries the last ABI error.</summary>
public sealed class EngineException : Exception
{
    public EngineException(PetsonaStatus status, string message)
        : base(string.IsNullOrEmpty(message)
            ? $"Petsona engine call failed with {status}."
            : message)
    {
        Status = status;
    }

    public PetsonaStatus Status { get; }
}

/// <summary>
/// Owns one Rust engine handle for one data directory. Create, use and
/// dispose the client on the same thread; the runtime worker owns all disk
/// and protocol work.
/// </summary>
public sealed class EngineClient : IDisposable
{
    private readonly EngineHandle _handle;
    private bool _disposed;

    public EngineClient(string? home = null)
    {
        unsafe
        {
            var homeBytes = home is null ? [] : Encoding.UTF8.GetBytes(home);
            fixed (byte* homePointer = homeBytes)
            {
                var options = new PetsonaEngineOptions
                {
                    AbiVersion = NativeMethods.AbiVersion,
                    Home = new PetsonaStringView
                    {
                        Ptr = homeBytes.Length == 0 ? null : homePointer,
                        Len = (nuint)homeBytes.Length,
                    },
                };
                nint handle = 0;
                var status = NativeMethods.petsona_engine_create(&options, &handle);
                if (status != PetsonaStatus.Ok || handle == 0)
                {
                    throw new EngineException(status, LastError());
                }

                _handle = new EngineHandle(handle);
            }
        }
    }

    /// <summary>Wake the runtime worker and advance timers as soon as possible.</summary>
    public PetsonaStatus Tick()
    {
        ThrowIfDisposed();
        _handle.AssertOwnerThread();
        return NativeMethods.petsona_engine_tick(_handle.Value);
    }

    /// <summary>Read the current immutable runtime projection.</summary>
    public PetsonaSnapshot Snapshot()
    {
        ThrowIfDisposed();
        _handle.AssertOwnerThread();
        PetsonaSnapshot snapshot;
        unsafe
        {
            var status = NativeMethods.petsona_engine_snapshot(_handle.Value, &snapshot);
            if (status != PetsonaStatus.Ok)
            {
                throw new EngineException(status, LastError());
            }
        }

        return snapshot;
    }

    /// <summary>Copy one UTF-8 text field; empty when the field is unset.</summary>
    public string Text(PetsonaTextField field)
    {
        ThrowIfDisposed();
        _handle.AssertOwnerThread();
        unsafe
        {
            var required = (int)NativeMethods.petsona_engine_copy_text(_handle.Value, (uint)field, null, 0);
            if (required <= 0)
            {
                return string.Empty;
            }

            var buffer = new byte[required];
            fixed (byte* destination = buffer)
            {
                var copied = (int)NativeMethods.petsona_engine_copy_text(
                    _handle.Value, (uint)field, destination, (nuint)buffer.Length);
                return Encoding.UTF8.GetString(buffer, 0, Math.Min(copied, buffer.Length));
            }
        }
    }

    /// <summary>Enqueue one validated command; text is copied before the call returns.</summary>
    public PetsonaStatus Send(
        PetsonaCommandKind kind,
        double value = 0,
        ulong ttlMilliseconds = 0,
        string text = "")
    {
        return SendRaw((uint)kind, value, ttlMilliseconds, text);
    }

    /// <summary>Enqueue a command with a raw kind ordinal (used by ABI tests).</summary>
    public PetsonaStatus SendRaw(
        uint kind,
        double value = 0,
        ulong ttlMilliseconds = 0,
        string text = "")
    {
        ThrowIfDisposed();
        _handle.AssertOwnerThread();
        var textBytes = Encoding.UTF8.GetBytes(text);
        unsafe
        {
            fixed (byte* textPointer = textBytes)
            {
                var command = new PetsonaCommand
                {
                    Kind = kind,
                    Reserved = 0,
                    Value = value,
                    TtlMs = ttlMilliseconds,
                    Text = new PetsonaStringView
                    {
                        Ptr = textBytes.Length == 0 ? null : textPointer,
                        Len = (nuint)textBytes.Length,
                    },
                };
                return NativeMethods.petsona_engine_command(_handle.Value, &command);
            }
        }
    }

    /// <summary>Read the calling thread's last ABI error message.</summary>
    public static string LastError()
    {
        unsafe
        {
            var required = (int)NativeMethods.petsona_last_error_copy(null, 0);
            if (required <= 0)
            {
                return string.Empty;
            }

            var buffer = new byte[required];
            fixed (byte* destination = buffer)
            {
                var copied = (int)NativeMethods.petsona_last_error_copy(destination, (nuint)buffer.Length);
                return Encoding.UTF8.GetString(buffer, 0, Math.Min(copied, buffer.Length));
            }
        }
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        if (_handle.IsValid)
        {
            _handle.AssertOwnerThread();
            NativeMethods.petsona_engine_destroy(_handle.Release());
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }
}
