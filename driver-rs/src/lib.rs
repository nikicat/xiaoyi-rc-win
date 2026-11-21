//! A UMDF filter that fixes the XiaoYi_RC remote's report descriptor in the
//! IOCTL replies, so that Windows accepts the remote (ADR 1). It sits above the
//! HidOverGatt minidriver (ADR 2) and is written on the `wdf` crate (ADR 6).

#![deny(unsafe_code)]

use std::sync::{Mutex, MutexGuard, PoisonError};

use wdf::{
    ctl_code, driver_entry, object_context, status_codes, Device, DeviceInit, Driver, DriverConfig,
    DriverObject, IoQueue, IoQueueConfig, IoQueueDispatchType, IoTarget, MemoryDescriptorMut,
    NtResult, NtStatus, Request, UnicodeString, FILE_ANY_ACCESS, METHOD_NEITHER,
};
use xiaoyi_rc_report::HidDescriptor;

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
    Other,
}

impl Ioctl {
    const GET_DEVICE_DESCRIPTOR: u32 = hid_ctl_code(0);
    const GET_REPORT_DESCRIPTOR: u32 = hid_ctl_code(1);

    fn parse(code: u32) -> Self {
        match code {
            Self::GET_DEVICE_DESCRIPTOR => Self::GetDeviceDescriptor,
            Self::GET_REPORT_DESCRIPTOR => Self::GetReportDescriptor,
            _ => Self::Other,
        }
    }
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
    let filter = Filter { target: device.get_io_target(), context: FilterContext::get(device) };
    match Ioctl::parse(code) {
        Ioctl::GetDeviceDescriptor => filter.handle_get_device_descriptor(request),
        Ioctl::GetReportDescriptor => filter.handle_get_report_descriptor(request),
        Ioctl::Other => filter.forward_and_forget(request),
    }
}

struct Filter<'a> {
    target: &'a IoTarget,
    context: &'a FilterContext,
}

impl Filter<'_> {
    /// Sends the request to the driver below unchanged. Completes the request
    /// with the send's status when the send fails.
    fn forward_and_forget(&self, mut request: Request) {
        request.format_using_current_type();
        if let Err(request) = request.send_and_forget(self.target) {
            let status = request.get_status();
            request.complete(status);
        }
    }

    /// Completes the request with the driver below's reply, and holds the trimmed
    /// report descriptor whose length the reply now states (ADR 2). When nothing
    /// is trimmed, the filter holds none and the reply goes up as it came.
    fn handle_get_device_descriptor(&self, mut request: Request) {
        request.format_using_current_type();
        let status = request.send_synchronously(self.target);
        let information = request.get_information();

        let trimmed = completed_hid_descriptor(&mut request, status, information).and_then(|mut hid| self.read_trimmed_descriptor(&mut hid));
        *self.context.descriptor() = trimmed;

        request.complete_with_information(status, information);
    }

    /// Returns the report descriptor that `hid` states, read from the driver
    /// below and trimmed by [`HidDescriptor::trim`]. Returns None when the read
    /// fails or [`HidDescriptor::trim`] returns None.
    fn read_trimmed_descriptor(&self, hid: &mut HidDescriptor<'_>) -> Option<Vec<u8>> {
        let mut report = vec![0; usize::from(hid.report_length())];
        // The read goes down as a regular IOCTL, not an internal one (ADR 2).
        let bytes = self
            .target
            .send_ioctl_synchronously(
                None,
                Ioctl::GET_REPORT_DESCRIPTOR,
                None,
                Some(&mut MemoryDescriptorMut::Buffer(&mut report)),
                None,
            )
            .ok()?;
        hid.trim(report.get(..bytes)?).map(<[u8]>::to_vec)
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

/// Returns the HID descriptor in the reply to IOCTL_HID_GET_DEVICE_DESCRIPTOR.
/// Changes to it change the reply. Returns None when the request failed or
/// [`HidDescriptor::parse`] finds no HID descriptor in the reply.
fn completed_hid_descriptor(request: &mut Request, status: NtStatus, information: usize) -> Option<HidDescriptor<'_>> {
    if !status.is_success() {
        return None;
    }
    let memory = request.retrieve_output_memory().ok()?;
    memory.get_buffer_mut().get_mut(..information).and_then(HidDescriptor::parse)
}
