//! A UMDF filter that fixes the XiaoYi_RC remote's report descriptor in the
//! IOCTL replies, so that Windows accepts the remote (ADR 1), and remaps its
//! buttons from the registry. It sits above the HidOverGatt minidriver (ADR 2)
//! and is written on the `wdf` crate (ADR 6).

#![deny(unsafe_code)]

use std::sync::{Mutex, MutexGuard, PoisonError};

use wdf::{
    ctl_code, driver_entry, object_context, status_codes, Device, DeviceInit, DeviceInstanceKeyType,
    Driver, DriverConfig, DriverObject, IoQueue, IoQueueConfig, IoQueueDispatchType, IoStatusBlock,
    IoTarget, MemoryDescriptorMut, NtResult, NtStatus, RegistryAccessRights, RegistryKey, Request,
    UnicodeString, UnicodeStringBuf, FILE_ANY_ACCESS, METHOD_NEITHER,
};
use xiaoyi_rc_report::{remap_target, HidDescriptor, InputReport, Remap, USAGES};

#[object_context(Device)]
struct FilterContext {
    /// The report descriptor with its trailing zeros dropped, which hidclass gets
    /// in place of the minidriver's (ADR 2).
    descriptor: Mutex<Option<Vec<u8>>>,
}

impl FilterContext {
    fn descriptor(&self) -> MutexGuard<'_, Option<Vec<u8>>> {
        // With panic = "abort" no holder can poison the lock.
        self.descriptor.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

const FILE_DEVICE_KEYBOARD: u32 = 0x0B;

/// HID_CTL_CODE in hidport.h.
const fn hid_ctl_code(function: u32) -> u32 {
    ctl_code(FILE_DEVICE_KEYBOARD, function, METHOD_NEITHER, FILE_ANY_ACCESS)
}

/// The device control requests that the filter changes. The rest pass down.
enum Ioctl {
    GetDeviceDescriptor,
    GetReportDescriptor,
    ReadReport,
    Other,
}

impl Ioctl {
    const GET_DEVICE_DESCRIPTOR: u32 = hid_ctl_code(0);
    const GET_REPORT_DESCRIPTOR: u32 = hid_ctl_code(1);
    const READ_REPORT: u32 = hid_ctl_code(2);

    fn parse(code: u32) -> Self {
        match code {
            Self::GET_DEVICE_DESCRIPTOR => Self::GetDeviceDescriptor,
            Self::GET_REPORT_DESCRIPTOR => Self::GetReportDescriptor,
            Self::READ_REPORT => Self::ReadReport,
            _ => Self::Other,
        }
    }
}

/// The devnode's `Device Parameters\XiaoYiRcFilter` key, named after the
/// filter's UMDF service. It holds the `Remap<usage>` values that configure.ps1
/// writes and the diagnostics that the filter records (ADR 2).
struct Parameters(RegistryKey);

impl Parameters {
    /// Returns the key, or None when it cannot be opened with `access`.
    fn open(device: &Device, access: RegistryAccessRights) -> Option<Self> {
        device.open_registry_key(DeviceInstanceKeyType::DeviceSubkey, access).ok().map(Self)
    }

    /// Returns the DWORD value `name`, or None when the key holds no such DWORD.
    fn get(&self, name: &str) -> Option<u32> {
        self.0.query_u32(&UnicodeStringBuf::from_str(name).ok()?).ok()
    }

    /// Sets the DWORD value `name`. A failed write goes unreported, since the
    /// filter writes only diagnostics.
    fn set(&self, name: &str, value: u32) {
        if let Ok(name) = UnicodeStringBuf::from_str(name) {
            let _ = self.0.assign_u32(&name, value);
        }
    }

    /// Returns the button map that the `Remap<usage>` values describe. Each
    /// value is the usage that its button sends instead, as [`remap_target`]
    /// reads it, and a value whose usage the report does not carry is ignored.
    /// For example, `RemapE9` = 0xCD sends Play/Pause for Volume Up.
    fn remap(&self) -> Remap {
        let mut remap = Remap::default();
        for usage in USAGES {
            if let Some(value) = self.get(&format!("Remap{usage:X}")) {
                let _ = remap.set(usage, remap_target(value));
            }
        }
        remap
    }
}

/// What reading the report descriptor found, which the filter records for
/// diagnosis (ADR 2).
struct DescriptorRead {
    /// The report descriptor length that the HID descriptor states.
    original_length: u16,
    /// The driver below's reply, or why the read failed.
    reply: NtResult<DescriptorReply>,
}

enum DescriptorReply {
    /// The driver below returned fewer bytes than the HID descriptor states.
    Short { bytes: usize },
    /// The full report descriptor, with nothing to trim.
    Untrimmed,
    /// The full report descriptor, trimmed.
    Trimmed(Vec<u8>),
}

impl DescriptorRead {
    /// Records the read under the value names that driver/driver.c uses.
    fn record(&self, parameters: &Parameters) {
        parameters.set("XiaoYiRcOriginalLength", self.original_length.into());
        parameters.set("XiaoYiRcReadStatus", dword((&self.reply).into()));
        // The trimmed length is 0 when the read failed or was short, as driver/driver.c records it.
        let (read_bytes, trimmed_length) = match &self.reply {
            Ok(DescriptorReply::Short { bytes }) => (saturating_dword(*bytes), 0),
            Ok(DescriptorReply::Untrimmed) => (self.original_length.into(), self.original_length.into()),
            Ok(DescriptorReply::Trimmed(trimmed)) => (self.original_length.into(), saturating_dword(trimmed.len())),
            Err(_) => (0, 0),
        };
        parameters.set("XiaoYiRcReadBytes", read_bytes);
        parameters.set("XiaoYiRcTrimmedLength", trimmed_length);
    }
}

/// Returns `status` as the DWORD that the registry records.
fn dword(status: NtStatus) -> u32 {
    status.code().cast_unsigned()
}

fn saturating_dword(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[driver_entry]
fn driver_entry(driver_object: &mut DriverObject, registry_path: &UnicodeString) -> NtResult<()> {
    Driver::create(driver_object, registry_path, DriverConfig::new(evt_device_add))?;
    Ok(())
}

fn evt_device_add(device_init: &mut DeviceInit) -> NtResult<()> {
    device_init.set_filter();
    let device = Device::create(device_init, None)?;
    FilterContext::attach(device, FilterContext { descriptor: Mutex::new(None) })?;
    IoQueue::create(device, &make_queue_config())?;
    Ok(())
}

/// Returns a queue config with only a device-control callback, since
/// mshidumdf delivers HID IOCTLs as regular device control (ADR 2, task Tpkpmd).
fn make_queue_config() -> IoQueueConfig {
    let mut queue_config = IoQueueConfig::new(IoQueueDispatchType::Parallel { presented_requests_limit: None });
    queue_config.default_queue = true;
    queue_config.evt_io_device_control = Some(evt_io_device_control);
    queue_config
}

fn evt_io_device_control(queue: &IoQueue, request: Request, _output_length: usize, _input_length: usize, code: u32) {
    let device = queue.get_device();
    let filter = Filter { device, target: device.get_io_target(), context: FilterContext::get(device) };
    match Ioctl::parse(code) {
        Ioctl::GetDeviceDescriptor => filter.handle_get_device_descriptor(request),
        Ioctl::GetReportDescriptor => filter.handle_get_report_descriptor(request),
        Ioctl::ReadReport => filter.forward_read_report(request),
        Ioctl::Other => filter.forward_and_forget(request),
    }
}

struct Filter<'a> {
    device: &'a Device,
    target: &'a IoTarget,
    context: &'a FilterContext,
}

impl Filter<'_> {
    /// Sends the request to the driver below unchanged. Completes the request
    /// with the send's status when the send fails.
    fn forward_and_forget(&self, mut request: Request) {
        request.format_using_current_type();
        if let Err(request) = request.send_and_forget(self.target) {
            complete_unsent(request);
        }
    }

    /// Sends the request to the driver below, with [`read_report_complete`] to
    /// remap the report that comes back. Completes the request with the error
    /// when that fails.
    fn forward_read_report(&self, mut request: Request) {
        request.format_using_current_type();
        if let Err(error) = request.set_completion_routine(read_report_complete) {
            return request.complete(error.into());
        }
        if let Err(request) = request.send_asynchronously(self.target) {
            complete_unsent(request);
        }
    }

    /// Completes the request with the driver below's reply, and holds the trimmed
    /// report descriptor whose length the reply now states (ADR 2). When nothing
    /// is trimmed, the filter holds none and the reply goes up as it came.
    /// Records the reply's status and the read for diagnosis.
    fn handle_get_device_descriptor(&self, mut request: Request) {
        request.format_using_current_type();
        let io_status =
            IoStatusBlock { status: request.send_synchronously(self.target), information: request.get_information() };

        let hid = completed_output(&mut request, &io_status).and_then(HidDescriptor::parse);
        let read = hid.map(|mut hid| self.read_descriptor(&mut hid));
        if let Some(parameters) = Parameters::open(self.device, RegistryAccessRights::SetValue) {
            parameters.set("XiaoYiRcDeviceDescriptorStatus", dword(io_status.status));
            if let Some(read) = &read {
                read.record(&parameters);
            }
        }
        *self.context.descriptor() = match read.map(|read| read.reply) {
            Some(Ok(DescriptorReply::Trimmed(trimmed))) => Some(trimmed),
            _ => None,
        };

        request.complete_with_information(io_status.status, io_status.information);
    }

    /// Reads the report descriptor that `hid` states from the driver below, and
    /// trims it with [`HidDescriptor::trim`].
    fn read_descriptor(&self, hid: &mut HidDescriptor<'_>) -> DescriptorRead {
        let original_length = hid.report_length();
        let mut report = vec![0; usize::from(original_length)];
        // The read goes down as a regular IOCTL, not an internal one (ADR 2).
        let result = self.target.send_ioctl_synchronously(
            None,
            Ioctl::GET_REPORT_DESCRIPTOR,
            None,
            Some(&mut MemoryDescriptorMut::Buffer(&mut report)),
            None,
        );
        let reply = result.map(|bytes| {
            if bytes != report.len() {
                DescriptorReply::Short { bytes }
            } else {
                hid.trim(&report).map_or(DescriptorReply::Untrimmed, |trimmed| DescriptorReply::Trimmed(trimmed.to_vec()))
            }
        });
        DescriptorRead { original_length, reply }
    }

    /// Completes the request with the held trimmed report descriptor, without
    /// passing it down (ADR 2). Passes the request down unchanged while none is held.
    fn handle_get_report_descriptor(&self, mut request: Request) {
        let held = self.context.descriptor().clone();
        let Some(descriptor) = held else {
            return self.forward_and_forget(request);
        };
        match request.retrieve_output_memory().and_then(|memory| memory.copy_from_buffer(0, &descriptor)) {
            Ok(()) => request.complete_with_information(NtStatus::from(status_codes::STATUS_SUCCESS), descriptor.len()),
            Err(error) => request.complete(error.into()),
        }
    }
}

/// Completes a request that the driver below did not take, with the status of
/// the failed send.
fn complete_unsent(request: Request) {
    let status = request.get_status();
    request.complete(status);
}

/// Remaps a press in the report that the driver below completed, with the map
/// that the registry holds now, then completes the request with the driver
/// below's reply. Releases need no remap or registry read. A press reads the
/// map anew, so that registry changes apply without restarting the device.
fn read_report_complete(mut request: Request, target: &IoTarget) {
    // Under UMDF, get_completion_params reports status 0 and length 0 (task Tpkpmd).
    let io_status = IoStatusBlock { status: request.get_status(), information: request.get_information() };
    if let Some(mut report) = completed_output(&mut request, &io_status).and_then(InputReport::parse)
        && report.is_press()
    {
        let remap = Parameters::open(target.get_device(), RegistryAccessRights::Read).map(|p| p.remap());
        remap.unwrap_or_default().apply(&mut report);
    }
    request.complete_with_information(io_status.status, io_status.information);
}

/// Returns the reply in the output buffer of a request that the driver below
/// completed with `io_status`. Changes to it change the reply.
/// Returns None when the request failed.
fn completed_output<'a>(request: &'a mut Request, io_status: &IoStatusBlock) -> Option<&'a mut [u8]> {
    if !io_status.status.is_success() {
        return None;
    }
    request.retrieve_output_memory().ok()?.get_buffer_mut().get_mut(..io_status.information)
}
