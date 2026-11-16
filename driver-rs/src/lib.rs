//! UMDF filter for the XiaoYi_RC remote, above HidOverGatt (ADRs 2 and 6).
//!
//! For now it passes every request down unchanged; the descriptor trim and
//! the remap follow in milestone M2.

#![deny(unsafe_code)]

use wdf::{
    driver_entry, Device, DeviceInit, Driver, DriverConfig, DriverObject, IoQueue, IoQueueConfig,
    IoQueueDispatchType, NtResult, Request, UnicodeString,
};

#[driver_entry]
fn driver_entry(driver_object: &mut DriverObject, registry_path: &UnicodeString) -> NtResult<()> {
    Driver::create(driver_object, registry_path, DriverConfig::new(evt_device_add))?;
    Ok(())
}

fn evt_device_add(device_init: &mut DeviceInit) -> NtResult<()> {
    device_init.set_filter();
    let device = Device::create(device_init, None)?;

    // HID requests reach a UMDF filter as device control, internal or not.
    let mut queue_config = IoQueueConfig::new(IoQueueDispatchType::Parallel { presented_requests_limit: None });
    queue_config.default_queue = true;
    queue_config.evt_io_device_control = Some(evt_io_device_control);
    IoQueue::create(&device, &queue_config)?;
    Ok(())
}

fn evt_io_device_control(queue: &IoQueue, request: Request, _output_length: usize, _input_length: usize, _code: u32) {
    forward_and_forget(queue.get_device(), request);
}

/// Passes `request` to the next driver down, or completes it with the
/// framework's status if it cannot be sent.
fn forward_and_forget(device: &Device, mut request: Request) {
    request.format_using_current_type();
    if let Err(request) = request.send_and_forget(device.get_io_target()) {
        let status = request.get_status();
        request.complete(status);
    }
}
