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
    const int DIGCF_PRESENT = 0x2;
    const int DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_READ = 0x80000000;
    const uint FILE_SHARE_READ = 0x1;
    const uint FILE_SHARE_WRITE = 0x2;
    const uint OPEN_EXISTING = 3;
    // sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_W). The SDK packs it to 1 byte on
    // 32-bit Windows and to 8 bytes on 64-bit Windows.
    static readonly int DetailCbSize = IntPtr.Size == 8 ? 8 : 6;
    const int DevicePathOffset = 4;

    [DllImport("hid.dll")] static extern void HidD_GetHidGuid(out Guid HidGuid);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid ClassGuid, IntPtr Enumerator, IntPtr hwndParent, int Flags);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr DeviceInfoSet, IntPtr DeviceInfoData, ref Guid InterfaceClassGuid,
        int MemberIndex, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr DeviceInfoSet, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData,
        IntPtr DeviceInterfaceDetailData, int DeviceInterfaceDetailDataSize, out int RequiredSize, IntPtr DeviceInfoData);
    [DllImport("setupapi.dll")] static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFile(string lpFileName, uint dwDesiredAccess, uint dwShareMode, IntPtr lpSecurityAttributes,
        uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool ReadFile(SafeFileHandle hFile, byte[] lpBuffer, int nNumberOfBytesToRead, out int lpNumberOfBytesRead,
        IntPtr lpOverlapped);

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA { public int cbSize; public Guid InterfaceClassGuid; public int Flags; public IntPtr Reserved; }

    static List<string> Paths(string match) {
        var res = new List<string>();
        Guid hidGuid; HidD_GetHidGuid(out hidGuid);
        IntPtr set = SetupDiGetClassDevs(ref hidGuid, IntPtr.Zero, IntPtr.Zero, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        try {
            for (int i = 0; ; i++) {
                var iface = new SP_DEVICE_INTERFACE_DATA(); iface.cbSize = Marshal.SizeOf(iface);
                if (!SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref hidGuid, i, ref iface)) break;
                int req; SetupDiGetDeviceInterfaceDetail(set, ref iface, IntPtr.Zero, 0, out req, IntPtr.Zero);
                IntPtr buf = Marshal.AllocHGlobal(req);
                try {
                    Marshal.WriteInt32(buf, DetailCbSize);
                    if (SetupDiGetDeviceInterfaceDetail(set, ref iface, buf, req, out req, IntPtr.Zero)) {
                        string p = Marshal.PtrToStringUni(buf + DevicePathOffset);
                        if (p.IndexOf(match, StringComparison.OrdinalIgnoreCase) >= 0) res.Add(p);
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
        while (DateTime.UtcNow < deadline) {
            var paths = Paths(match);
            if (paths.Count == 0) {
                Thread.Sleep(200);
                continue;
            }
            string path = paths[0];
            var h = CreateFile(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
            if (h.IsInvalid) {
                Log("open failed: " + Marshal.GetLastWin32Error() + " " + path);
                Thread.Sleep(1000);
                continue;
            }
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
            bool ended = reader.Join((int)Math.Max(0, (deadline - DateTime.UtcNow).TotalMilliseconds));
            h.Dispose();
            if (ended) Log("disconnected");
        }
        Log("done");
    }
}
'@
[HidLog]::Run($Seconds, $Match)
