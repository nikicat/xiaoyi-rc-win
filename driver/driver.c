// UMDF 2 upper filter above HidOverGatt for the XiaoYi_RC remote. It makes the
// remote's padded report descriptor acceptable and remaps its buttons (ADR 2).

#include <windows.h>
#include <stdio.h>
#include <wdf.h>
#include <hidport.h>

#include "remap.h"
#include "trim.h"

#define MAX_REPORT_DESCRIPTOR 4096

typedef struct _FILTER_CONTEXT {
    // The filter's own device, which owns the registry keys.
    WDFDEVICE Device;
    // The HidOverGatt stack below, which gets every request the filter passes on.
    WDFIOTARGET Target;
    // The trimmed report descriptor that IOCTL_HID_GET_REPORT_DESCRIPTOR is
    // answered from.
    UCHAR Descriptor[MAX_REPORT_DESCRIPTOR];
    // Length of the trimmed copy in Descriptor, or 0 while none is held.
    ULONG TrimmedLength;
} FILTER_CONTEXT, *PFILTER_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(FILTER_CONTEXT, GetFilterContext)

// The outcome of reading the minidriver's report descriptor and trimming it.
typedef struct _DESCRIPTOR_READ {
    // The descriptor length that the minidriver's HID_DESCRIPTOR states.
    ULONG OriginalLength;
    NTSTATUS Status;
    ULONG Bytes;
    // The length without the padding, or 0 when the read failed.
    ULONG TrimmedLength;
} DESCRIPTOR_READ;

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD EvtDeviceAdd;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL EvtIoDeviceControl;

// Opens the devnode's Device Parameters\XiaoYiRcFilter key, which holds the
// filter's settings and diagnostics.
static NTSTATUS OpenParametersKey(WDFDEVICE Device, ACCESS_MASK Access, WDFKEY *Key)
{
    return WdfDeviceOpenRegistryKey(Device, PLUGPLAY_REGKEY_DEVICE | WDF_REGKEY_DEVICE_SUBKEY,
                                    Access, WDF_NO_OBJECT_ATTRIBUTES, Key);
}

// Leave a trace under the devnode's Device Parameters for diagnosis.
static void Record(WDFDEVICE Device, PCWSTR Name, ULONG Value)
{
    WDFKEY key;
    UNICODE_STRING name;

    if (NT_SUCCESS(OpenParametersKey(Device, KEY_SET_VALUE, &key))) {
        RtlInitUnicodeString(&name, Name);
        WdfRegistryAssignULong(key, &name, Value);
        WdfRegistryClose(key);
    }
}

// Builds the button map from the DWORD values under the devnode's Device
// Parameters. A value named Remap<usage hex> holds the usage that button sends
// instead, as RemapSet takes it. For example, RemapE9 = 0xCD sends Play/Pause
// for Volume Up. configure.ps1 writes these values, and
// tests/protocol_test.ps1 checks that the two agree.
static void LoadRemap(WDFDEVICE Device, REMAP_MAP *Map)
{
    WDFKEY key;
    WCHAR nameBuf[16];
    UNICODE_STRING name;
    ULONG target;
    int i;

    RemapInit(Map);
    if (!NT_SUCCESS(OpenParametersKey(Device, KEY_READ, &key))) {
        return;
    }
    for (i = 0; i < REMAP_BUTTONS; i++) {
        swprintf_s(nameBuf, ARRAYSIZE(nameBuf), L"Remap%X", RemapUsages[i]);
        RtlInitUnicodeString(&name, nameBuf);
        if (NT_SUCCESS(WdfRegistryQueryULong(key, &name, &target))) {
            RemapSet(Map, RemapUsages[i], target);
        }
    }
    WdfRegistryClose(key);
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
    PFILTER_CONTEXT ctx;
    NTSTATUS status;

    UNREFERENCED_PARAMETER(Driver);

    WdfFdoInitSetFilter(DeviceInit);

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, FILTER_CONTEXT);
    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) {
        return status;
    }
    ctx = GetFilterContext(device);
    ctx->Device = device;
    ctx->Target = WdfDeviceGetIoTarget(device);

    // mshidumdf delivers hidclass's internal IOCTLs as regular device control
    // (ADR 2). The internal callback is a safety net in case that changes.
    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchParallel);
    queueConfig.EvtIoDeviceControl = EvtIoDeviceControl;
    queueConfig.EvtIoInternalDeviceControl = EvtIoDeviceControl;
    return WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
}

// Sends the request on to Target as it came in, and completes it with the
// send's status when the send fails. Completion, when given, runs when the
// target completes the request.
static void SendOrComplete(WDFREQUEST Request, WDFIOTARGET Target, PWDF_REQUEST_SEND_OPTIONS Options,
                           PFN_WDF_REQUEST_COMPLETION_ROUTINE Completion)
{
    WdfRequestFormatRequestUsingCurrentType(Request);
    if (Completion != NULL) {
        WdfRequestSetCompletionRoutine(Request, Completion, WDF_NO_CONTEXT);
    }
    if (!WdfRequestSend(Request, Target, Options)) {
        WdfRequestComplete(Request, WdfRequestGetStatus(Request));
    }
}

static void ForwardAndForget(WDFREQUEST Request, WDFIOTARGET Target)
{
    WDF_REQUEST_SEND_OPTIONS options;

    WDF_REQUEST_SEND_OPTIONS_INIT(&options, WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET);
    SendOrComplete(Request, Target, &options, NULL);
}

static BOOLEAN HasTrimmedDescriptor(const FILTER_CONTEXT *Ctx)
{
    return Ctx->TrimmedLength != 0;
}

// Checks whether the read succeeded and trimming shortened the descriptor.
static BOOLEAN DescriptorWasTrimmed(const DESCRIPTOR_READ *Read)
{
    return Read->TrimmedLength != 0 && Read->TrimmedLength != Read->OriginalLength;
}

// Reads the minidriver's untrimmed report descriptor into Ctx->Descriptor and
// trims it. The status is STATUS_INVALID_BUFFER_SIZE when OriginalLength is 0
// or does not fit the context.
static DESCRIPTOR_READ ReadTrimmedDescriptor(PFILTER_CONTEXT Ctx, ULONG OriginalLength)
{
    DESCRIPTOR_READ read = { OriginalLength, STATUS_INVALID_BUFFER_SIZE, 0, 0 };
    WDF_MEMORY_DESCRIPTOR output;
    ULONG_PTR bytes = 0;

    if (OriginalLength == 0 || OriginalLength > sizeof(Ctx->Descriptor)) {
        return read;
    }

    WDF_MEMORY_DESCRIPTOR_INIT_BUFFER(&output, Ctx->Descriptor, OriginalLength);
    // UMDF cannot send internal IOCTLs, so this goes down as a regular one.
    read.Status = WdfIoTargetSendIoctlSynchronously(Ctx->Target, WDF_NO_HANDLE,
                                                    IOCTL_HID_GET_REPORT_DESCRIPTOR,
                                                    NULL, &output, NULL, &bytes);
    read.Bytes = (ULONG)bytes;
    if (NT_SUCCESS(read.Status) && read.Bytes == OriginalLength) {
        read.TrimmedLength = (ULONG)TrimReportDescriptor(Ctx->Descriptor, OriginalLength);
    }
    return read;
}

static void RecordDescriptorRead(WDFDEVICE Device, const DESCRIPTOR_READ *Read)
{
    Record(Device, L"XiaoYiRcOriginalLength", Read->OriginalLength);
    Record(Device, L"XiaoYiRcReadStatus", (ULONG)Read->Status);
    Record(Device, L"XiaoYiRcReadBytes", Read->Bytes);
    Record(Device, L"XiaoYiRcTrimmedLength", Read->TrimmedLength);
}

// Returns the HID_DESCRIPTOR that a completed IOCTL_HID_GET_DEVICE_DESCRIPTOR
// carries, or NULL when the request failed or returned no report descriptor.
static PHID_DESCRIPTOR CompletedHidDescriptor(WDFREQUEST Request)
{
    WDFMEMORY memory;
    PHID_DESCRIPTOR hidDesc;

    if (!NT_SUCCESS(WdfRequestGetStatus(Request)) ||
        WdfRequestGetInformation(Request) < sizeof(HID_DESCRIPTOR) ||
        !NT_SUCCESS(WdfRequestRetrieveOutputMemory(Request, &memory))) {
        return NULL;
    }
    hidDesc = (PHID_DESCRIPTOR)WdfMemoryGetBuffer(memory, NULL);
    return hidDesc->bNumDescriptors >= 1 ? hidDesc : NULL;
}

static void HandleGetDeviceDescriptor(PFILTER_CONTEXT Ctx, WDFREQUEST Request)
{
    WDF_REQUEST_SEND_OPTIONS options;
    PHID_DESCRIPTOR hidDesc;
    DESCRIPTOR_READ read;
    NTSTATUS status;

    WdfRequestFormatRequestUsingCurrentType(Request);
    WDF_REQUEST_SEND_OPTIONS_INIT(&options, WDF_REQUEST_SEND_OPTION_SYNCHRONOUS);
    WdfRequestSend(Request, Ctx->Target, &options);
    status = WdfRequestGetStatus(Request);
    Record(Ctx->Device, L"XiaoYiRcDeviceDescriptorStatus", (ULONG)status);

    hidDesc = CompletedHidDescriptor(Request);
    if (hidDesc != NULL) {
        Ctx->TrimmedLength = 0;
        read = ReadTrimmedDescriptor(Ctx, hidDesc->DescriptorList[0].wReportLength);
        RecordDescriptorRead(Ctx->Device, &read);
        if (DescriptorWasTrimmed(&read)) {
            Ctx->TrimmedLength = read.TrimmedLength;
            hidDesc->DescriptorList[0].wReportLength = (USHORT)read.TrimmedLength;
        }
    }

    WdfRequestCompleteWithInformation(Request, status, WdfRequestGetInformation(Request));
}

static void HandleGetReportDescriptor(PFILTER_CONTEXT Ctx, WDFREQUEST Request)
{
    WDFMEMORY memory;
    NTSTATUS status;

    if (!HasTrimmedDescriptor(Ctx)) {
        ForwardAndForget(Request, Ctx->Target);
        return;
    }

    status = WdfRequestRetrieveOutputMemory(Request, &memory);
    if (NT_SUCCESS(status)) {
        status = WdfMemoryCopyFromBuffer(memory, 0, Ctx->Descriptor, Ctx->TrimmedLength);
    }
    WdfRequestCompleteWithInformation(Request, status, NT_SUCCESS(status) ? Ctx->TrimmedLength : 0);
}

static void ReadReportComplete(WDFREQUEST Request, WDFIOTARGET Target,
                               PWDF_REQUEST_COMPLETION_PARAMS Params, WDFCONTEXT Context)
{
    WDFDEVICE device = WdfIoTargetGetDevice(Target);
    NTSTATUS status = Params->IoStatus.Status;
    ULONG_PTR length = Params->IoStatus.Information;
    REMAP_MAP map;
    WDFMEMORY memory;
    unsigned char *report;
    size_t size;

    UNREFERENCED_PARAMETER(Context);

    if (NT_SUCCESS(status) && NT_SUCCESS(WdfRequestRetrieveOutputMemory(Request, &memory))) {
        report = (unsigned char *)WdfMemoryGetBuffer(memory, &size);
        // Releases need no remap or registry read. A press reads the map anew
        // so that registry changes apply without restarting the device.
        if (length <= size && RemapIsPress(report, length)) {
            LoadRemap(device, &map);
            RemapReport(&map, report, length);
        }
    }
    WdfRequestCompleteWithInformation(Request, status, length);
}

static void ForwardReadReport(WDFREQUEST Request, WDFIOTARGET Target)
{
    SendOrComplete(Request, Target, WDF_NO_SEND_OPTIONS, ReadReportComplete);
}

// Takes both device control and internal device control requests (see
// EvtDeviceAdd).
VOID EvtIoDeviceControl(WDFQUEUE Queue, WDFREQUEST Request, size_t OutputBufferLength,
                        size_t InputBufferLength, ULONG IoControlCode)
{
    PFILTER_CONTEXT ctx = GetFilterContext(WdfIoQueueGetDevice(Queue));

    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);

    switch (IoControlCode) {
    case IOCTL_HID_GET_DEVICE_DESCRIPTOR:
        HandleGetDeviceDescriptor(ctx, Request);
        break;
    case IOCTL_HID_GET_REPORT_DESCRIPTOR:
        HandleGetReportDescriptor(ctx, Request);
        break;
    case IOCTL_HID_READ_REPORT:
        ForwardReadReport(Request, ctx->Target);
        break;
    default:
        ForwardAndForget(Request, ctx->Target);
        break;
    }
}
