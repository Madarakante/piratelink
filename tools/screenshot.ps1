# Captures the PirateLink window to a PNG for visual verification.
# Usage: powershell -ExecutionPolicy Bypass -File tools\screenshot.ps1 -Out shot.png

param(
    [string]$Out = "shot.png",
    [string]$ProcessName = "piratelink",
    [int]$WaitMs = 2500
)

Add-Type -AssemblyName System.Drawing

if (-not ("WinCap" -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinCap {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@
}

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) {
    Write-Error "No '$ProcessName' process with a window found."
    exit 1
}

[void][WinCap]::ShowWindow($proc.MainWindowHandle, 9)   # SW_RESTORE
[void][WinCap]::SetForegroundWindow($proc.MainWindowHandle)
Start-Sleep -Milliseconds $WaitMs

$rect = New-Object WinCap+RECT
if (-not [WinCap]::GetWindowRect($proc.MainWindowHandle, [ref]$rect)) {
    Write-Error "GetWindowRect failed."
    exit 1
}

$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
if ($w -le 0 -or $h -le 0) {
    Write-Error "Window has no size ($w x $h)."
    exit 1
}

$bmp = New-Object System.Drawing.Bitmap $w, $h
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
$gfx.Dispose()

$path = [System.IO.Path]::GetFullPath($Out)
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

Write-Output "$path ($w x $h)"
