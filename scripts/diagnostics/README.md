# Manual diagnostics

Small, **manual** probes kept next to the gates because they answer questions the
smoke cannot. They are not part of `verify-windows.ps1` / CI, and nothing else
depends on them.

Rules these scripts follow (keep it that way when editing):

- each run uses a throwaway `PETSONA_HOME` under `%TEMP%` plus a free port, so the
  user's real `%APPDATA%\Petsona` data is never touched;
- they only read the built app from
  `apps\windows\Petsona\bin\x64\Release\net10.0-windows10.0.26100.0\win-x64`
  (run `scripts\package-windows.ps1` or the verify gate first);
- they may move the physical cursor, so do not touch the mouse while they run.

| Script | Question it answers | Evidence |
|---|---|---|
| `cursor-return-probe.ps1` | Do the pet / overlay window classes own the cursor? Registers a control window with a NULL class cursor that must answer `0`, so `1` really means "the window installed a cursor" (smoke N23/N24 automate this) | `docs/execution/windows-native-rewrite.md` E-W15b |
| `startup-timing.ps1` | How long does `Start-Process` → first visible pet frame take (cold / hot)? | `docs/execution/windows-native-rewrite.md` E-W15d |

Historical: the `startup-trace.ps1` / `build-trace.ps1` scratch scripts drove a
temporary `PETSONA_STARTUP_TRACE` instrumentation inside `App.xaml.cs` /
`AppController.cs`. That instrumentation was rolled back byte-for-byte after the
measurement (see E-W15d), so those scripts were dropped; recreate the trace by
adding timestamped `File.AppendAllText` calls at the same three points if a future
investigation needs it.
