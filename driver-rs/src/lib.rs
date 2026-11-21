//! A UMDF filter for the XiaoYi_RC remote that sits above HidOverGatt (ADR 2),
//! ported to Rust on the `wdf` crate (ADR 6). It passes every request down
//! unchanged until milestone M2 adds the descriptor trim and the remap.

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
    IoQueue::create(&device, &make_queue_config())?;
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

fn evt_io_device_control(queue: &IoQueue, mut request: Request, _output_length: usize, _input_length: usize, _code: u32) {
    request.format_using_current_type();
    if let Err(request) = request.send_and_forget(queue.get_device().get_io_target()) {
        let status = request.get_status();
        request.complete(status);
    }
}
