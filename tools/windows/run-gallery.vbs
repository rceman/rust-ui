' run-gallery.vbs -- zero-flash launcher for the rust-ui Gallery.
'
' powershell.exe is a console-subsystem binary: even -WindowStyle Hidden
' creates a console window first and hides it after, which shows as a
' brief terminal flash (or a Windows Terminal tab on systems where WT is
' the default console host). wscript.exe is GUI-subsystem, so invoking
' the PowerShell script through it never creates any window at all --
' the gallery's own WndProc window is the first thing the user sees.
'
' Desktop shortcut target:
'   wscript.exe "<resolved repo UNC>\tools\windows\run-gallery.vbs"

Option Explicit

Dim shell, fso, repo, script, cmd

Set shell = CreateObject("WScript.Shell")
Set fso   = CreateObject("Scripting.FileSystemObject")

' Resolve the authoritative WSL checkout to its Windows UNC at runtime
' via the WSL tooling itself -- no distro name is hard-coded.
Dim exec : Set exec = shell.Exec("wsl.exe wslpath -w /home/therceman/git/rust-ui")
' ReadLine excludes the line terminator -- Trim alone leaves a stray CRLF
' that breaks the path concatenation below
repo = Trim(exec.StdOut.ReadLine())

If repo = "" Or Not fso.FileExists(repo & "\tools\windows\run-gallery.ps1") Then
    MsgBox "rust-ui Gallery: repo not reachable via wslpath -w /home/therceman/git/rust-ui", _
           vbCritical, "rust-ui Gallery - launch failed"
    WScript.Quit 1
End If

script = repo & "\tools\windows\run-gallery.ps1"
cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & script & """"
' 0 = hidden window, False = do not wait
shell.Run cmd, 0, False
