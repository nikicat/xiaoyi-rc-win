// UMDF 2 upper filter above HidOverGatt for the XiaoYi_RC remote (ADR 2).
//
// The remote's Report Map is a valid descriptor followed by 0x00 padding,
// which hidparse rejects. On IOCTL_HID_GET_DEVICE_DESCRIPTOR we read the
// full report descriptor ourselves, trim the padding, and patch
// wReportLength; IOCTL_HID_GET_REPORT_DESCRIPTOR is then answered from the
// trimmed copy. Everything else is forwarded untouched.

#include <windows.h>
#include <stdarg.h>
#include <stdio.h>
#include <wdf.h>
#include <hidport.h>

#include "trim.h"

#define MAX_REPORT_DESCRIPTOR 4096

typedef struct _FILTER_CONTEXT {
    UCHAR Descriptor[MAX_REPORT_DESCRIPTOR];
    ULONG Length;      // trimmed length; 0 until a trimmed copy is held
} FILTER_CONTEXT, *PFILTER_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(FILTER_CONTEXT, GetFilterContext)

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD EvtDeviceAdd;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL EvtIoDeviceControl;
EVT_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL EvtIoInternalDeviceControl;

static void Log(const char *fmt, ...)
{
    char buf[256];
    va_list args;

    va_start(args, fmt);
    _vsnprintf_s(buf, sizeof(buf), _TRUNCATE, fmt, args);
    va_end(args);
    OutputDebugStringA(buf);
}

// Leave a trace under the devnode's Device Parameters for diagnosis.
static void Record(WDFDEVICE Device, PCWSTR Name, ULONG Value)
{
    WDFKEY key;
    UNICODE_STRING name;

    if (NT_SUCCESS(WdfDeviceOpenRegistryKey(Device, PLUGPLAY_REGKEY_DEVICE | WDF_REGKEY_DEVICE_SUBKEY,
                                            KEY_SET_VALUE, WDF_NO_OBJECT_ATTRIBUTES, &key))) {
        RtlInitUnicodeString(&name, Name);
        WdfRegistryAssignULong(key, &name, Value);
        WdfRegistryClose(key);
    }
}

NTSTATUS DriverEntry(PDRIVER_OBJECT DriverObject, PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;

    WDF_DRIVER_CONFIG_INIT(&config, EvtDeviceAdd);
    return WdfDriverCreate(DriverObject, RegistryPath, WDF_NO_OBJECT_ATTRIBUTES,
                           &config, WDF_NO_HANDLE);
}

NTSTATUS EvtDeviceAdd(WDFDRIVER Driver, PWDFDEVICE_INIT DeviceInit)
{
    WDF_OBJECT_ATTRIBUTES attributes;
    WDF_IO_QUEUE_CONFIG queueConfig;
    WDFDEVICE device;
    NTSTATUS status;

    UNREFERENCED_PARAMETER(Driver);

    WdfFdoInitSetFilter(DeviceInit);

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, FILTER_CONTEXT);
    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) {
        return status;
    }
    Record(device, L"XiaoYiRcDeviceAdded", 1);

    // hidclass sends internal IOCTLs; take them whichever way mshidumdf
    // delivers them.
    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchParallel);
    queueConfig.EvtIoDeviceControl = EvtIoDeviceControl;
    queueConfig.EvtIoInternalDeviceControl = EvtIoInternalDeviceControl;
    return WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
}

static void ForwardAndForget(WDFREQUEST Request, WDFIOTARGET Target)
{
    WDF_REQUEST_SEND_OPTIONS options;

    WdfRequestFormatRequestUsingCurrentType(Request);
    WDF_REQUEST_SEND_OPTIONS_INIT(&options, WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET);
    if (!WdfRequestSend(Request, Target, &options)) {
        WdfRequestComplete(Request, WdfRequestGetStatus(Request));
    }
}

// Read the untrimmed report descriptor from the minidriver into the context
// and keep the trimmed length if trimming changes anything.
static void LoadTrimmedDescriptor(WDFDEVICE Device, WDFIOTARGET Target, BOOLEAN Internal,
                                  PFILTER_CONTEXT Ctx, ULONG OrigLength)
{
    WDF_MEMORY_DESCRIPTOR output;
    ULONG_PTR bytes = 0;
    ULONG trimmed;
    NTSTATUS status;

    Ctx->Length = 0;
    Record(Device, L"XiaoYiRcOriginalLength", OrigLength);
    if (OrigLength == 0 || OrigLength > sizeof(Ctx->Descriptor)) {
        return;
    }

    WDF_MEMORY_DESCRIPTOR_INIT_BUFFER(&output, Ctx->Descriptor, OrigLength);
    // UMDF cannot send internal IOCTLs, so this goes down as a regular one.
    UNREFERENCED_PARAMETER(Internal);
    status = WdfIoTargetSendIoctlSynchronously(Target, WDF_NO_HANDLE,
                                               IOCTL_HID_GET_REPORT_DESCRIPTOR,
                                               NULL, &output, NULL, &bytes);
    Record(Device, L"XiaoYiRcReadStatus", (ULONG)status);
    Record(Device, L"XiaoYiRcReadBytes", (ULONG)bytes);
    if (!NT_SUCCESS(status) || bytes != OrigLength) {
        Log("xiaoyi-rc: own report descriptor read failed: 0x%08X, %Iu bytes\n",
            status, bytes);
        return;
    }

    trimmed = (ULONG)TrimReportDescriptor(Ctx->Descriptor, OrigLength);
    Log("xiaoyi-rc: report descriptor %lu -> %lu bytes\n", OrigLength, trimmed);
    Record(Device, L"XiaoYiRcTrimmedLength", trimmed);
    if (trimmed != OrigLength) {
        Ctx->Length = trimmed;
    }
}

static void HandleGetDeviceDescriptor(WDFDEVICE Device, WDFREQUEST Request, WDFIOTARGET Target,
                                      BOOLEAN Internal, PFILTER_CONTEXT Ctx)
{
    WDF_REQUEST_SEND_OPTIONS options;
    WDFMEMORY memory;
    size_t size;
    PHID_DESCRIPTOR hidDesc;
    NTSTATUS status;

    WdfRequestFormatRequestUsingCurrentType(Request);
    WDF_REQUEST_SEND_OPTIONS_INIT(&options, WDF_REQUEST_SEND_OPTION_SYNCHRONOUS);
    WdfRequestSend(Request, Target, &options);
    status = WdfRequestGetStatus(Request);
    Record(Device, L"XiaoYiRcDeviceDescriptorStatus", (ULONG)status);

    if (NT_SUCCESS(status) &&
        NT_SUCCESS(WdfRequestRetrieveOutputMemory(Request, &memory))) {
        hidDesc = (PHID_DESCRIPTOR)WdfMemoryGetBuffer(memory, &size);
        if (size >= sizeof(HID_DESCRIPTOR) && hidDesc->bNumDescriptors >= 1) {
            LoadTrimmedDescriptor(Device, Target, Internal, Ctx,
                                  hidDesc->DescriptorList[0].wReportLength);
            if (Ctx->Length != 0) {
                hidDesc->DescriptorList[0].wReportLength = (USHORT)Ctx->Length;
            }
        }
    }

    WdfRequestCompleteWithInformation(Request, status, WdfRequestGetInformation(Request));
}

static void HandleGetReportDescriptor(WDFREQUEST Request, WDFIOTARGET Target, PFILTER_CONTEXT Ctx)
{
    WDFMEMORY memory;
    NTSTATUS status;

    if (Ctx->Length == 0) {
        ForwardAndForget(Request, Target);
        return;
    }

    status = WdfRequestRetrieveOutputMemory(Request, &memory);
    if (NT_SUCCESS(status)) {
        status = WdfMemoryCopyFromBuffer(memory, 0, Ctx->Descriptor, Ctx->Length);
    }
    WdfRequestCompleteWithInformation(Request, status, NT_SUCCESS(status) ? Ctx->Length : 0);
}

static void DispatchIoctl(WDFQUEUE Queue, WDFREQUEST Request, ULONG IoControlCode, BOOLEAN Internal)
{
    WDFDEVICE device = WdfIoQueueGetDevice(Queue);
    WDFIOTARGET target = WdfDeviceGetIoTarget(device);
    PFILTER_CONTEXT ctx = GetFilterContext(device);

    switch (IoControlCode) {
    case IOCTL_HID_GET_DEVICE_DESCRIPTOR:
        Record(device, L"XiaoYiRcDeviceDescriptorInternal", Internal);
        HandleGetDeviceDescriptor(device, Request, target, Internal, ctx);
        break;
    case IOCTL_HID_GET_REPORT_DESCRIPTOR:
        HandleGetReportDescriptor(Request, target, ctx);
        break;
    default:
        ForwardAndForget(Request, target);
        break;
    }
}

VOID EvtIoDeviceControl(WDFQUEUE Queue, WDFREQUEST Request, size_t OutputBufferLength,
                        size_t InputBufferLength, ULONG IoControlCode)
{
    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);
    DispatchIoctl(Queue, Request, IoControlCode, FALSE);
}

VOID EvtIoInternalDeviceControl(WDFQUEUE Queue, WDFREQUEST Request, size_t OutputBufferLength,
                                size_t InputBufferLength, ULONG IoControlCode)
{
    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);
    DispatchIoctl(Queue, Request, IoControlCode, TRUE);
}
