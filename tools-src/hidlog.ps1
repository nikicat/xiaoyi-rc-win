# Log the remote's raw HID input reports with timestamps, and when its HID
# collection appears or goes away (the remote connecting / sleeping).
# Usage: .\tools-src\hidlog.ps1 [-Seconds 120]
param([int]$Seconds = 120, [string]$Match = 'vid&0100d2_pid&0580')

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading;
using Microsoft.Win32.SafeHandles;

public static class HidLog {
    [DllImport("hid.dll")] static extern void HidD_GetHidGuid(out Guid g);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid g, IntPtr e, IntPtr h, int flags);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr s, IntPtr d, ref Guid g, int i, ref IFACE data);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr s, ref IFACE data, IntPtr detail, int size, out int req, IntPtr dev);
    [DllImport("setupapi.dll")] static extern bool SetupDiDestroyDeviceInfoList(IntPtr s);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFile(string n, uint access, uint share, IntPtr sa, uint disp, uint flags, IntPtr t);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool ReadFile(SafeFileHandle h, byte[] buf, int n, out int read, IntPtr ov);

    [StructLayout(LayoutKind.Sequential)]
    struct IFACE { public int cbSize; public Guid g; public int flags; public IntPtr r; }

    static List<string> Paths(string match) {
        var res = new List<string>();
        Guid g; HidD_GetHidGuid(out g);
        IntPtr set = SetupDiGetClassDevs(ref g, IntPtr.Zero, IntPtr.Zero, 0x12); // PRESENT | DEVICEINTERFACE
        try {
            for (int i = 0; ; i++) {
                var d = new IFACE(); d.cbSize = Marshal.SizeOf(d);
                if (!SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref g, i, ref d)) break;
                int req; SetupDiGetDeviceInterfaceDetail(set, ref d, IntPtr.Zero, 0, out req, IntPtr.Zero);
                IntPtr buf = Marshal.AllocHGlobal(req);
                try {
                    Marshal.WriteInt32(buf, IntPtr.Size == 8 ? 8 : 6);
                    if (SetupDiGetDeviceInterfaceDetail(set, ref d, buf, req, out req, IntPtr.Zero)) {
                        string p = Marshal.PtrToStringUni(buf + 4);
                        if (p.ToLowerInvariant().Contains(match)) res.Add(p);
                    }
                } finally { Marshal.FreeHGlobal(buf); }
            }
        } finally { SetupDiDestroyDeviceInfoList(set); }
        return res;
    }

    static void Log(string s) {
        Console.WriteLine(DateTime.Now.ToString("HH:mm:ss.fff ") + s);
        Console.Out.Flush();
    }

    public static void Run(int seconds, string match) {
        var deadline = DateTime.UtcNow.AddSeconds(seconds);
        string open = null;
        while (DateTime.UtcNow < deadline) {
            var paths = Paths(match);
            if (paths.Count == 0) {
                if (open != null) { Log("disconnected"); open = null; }
                Thread.Sleep(200);
                continue;
            }
            string path = paths[0];
            var h = CreateFile(path, 0x80000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero); // GENERIC_READ, share RW
            if (h.IsInvalid) {
                Log("open failed: " + Marshal.GetLastWin32Error() + " " + path);
                Thread.Sleep(1000);
                continue;
            }
            open = path;
            Log("connected " + path);
            var reader = new Thread(() => {
                var buf = new byte[64];
                int n;
                while (ReadFile(h, buf, buf.Length, out n, IntPtr.Zero))
                    Log("report " + BitConverter.ToString(buf, 0, n));
                Log("read ended: " + Marshal.GetLastWin32Error());
            });
            reader.IsBackground = true;
            reader.Start();
            while (reader.IsAlive && DateTime.UtcNow < deadline) Thread.Sleep(100);
            h.Dispose();
            if (!reader.IsAlive) { Log("disconnected"); open = null; }
        }
        Log("done");
    }
}
'@
[HidLog]::Run($Seconds, $Match)
