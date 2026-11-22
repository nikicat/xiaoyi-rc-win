//! A UMDF filter above the HidOverGatt minidriver (ADR 2), on the `wdf` crate
//! (ADR 6). It fixes the remote's report descriptor so that Windows accepts the
//! remote (ADR 1), and turns its button taps into gestures (ADR 7).

#![deny(unsafe_code)]

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use wdf::{
    Arc, Device, DeviceInit, DeviceInstanceKeyType, Driver, DriverConfig, DriverObject,
    FILE_ANY_ACCESS, IoQueue, IoQueueConfig, IoQueueDispatchType, IoStatusBlock, IoTarget,
    METHOD_NEITHER, MemoryDescriptorMut, NtResult, NtStatus, RegistryAccessRights, RegistryKey,
    Request, RequestId, SentRequestCancellationToken, Timer, TimerConfig, UnicodeString,
    UnicodeStringBuf, ctl_code, driver_entry, object_context, status_codes,
};
use xiaoyi_rc_report::{
    Action, Bindings, Config, HidDescriptor, InputReport, ReadState, SerialCompleter, Time, Token,
};

#[object_context(Device)]
struct FilterContext {
    /// The report descriptor with its trailing zeros dropped, which hidclass gets
    /// in place of the minidriver's (ADR 2).
    descriptor: Mutex<Option<Vec<u8>>>,
    reads: ReadPath,
}

impl FilterContext {
    fn descriptor(&self) -> MutexGuard<'_, Option<Vec<u8>>> {
        lock(&self.descriptor)
    }
}

/// Carries the reports of the gestures up in hidclass's reads, in place of the
/// remote's reports (ADR 7, "The read path").
struct ReadPath {
    completer: SerialCompleter<Request, ReadToken>,
    /// Fires when a click window ends.
    window_timer: Arc<Timer>,
    /// The clock that the gestures count from.
    started: Instant,
}

type Reads = ReadState<Request, ReadToken>;

type ReadAction = Action<Request, ReadToken>;

/// Cancels one of hidclass's reads that is down below.
struct ReadToken(SentRequestCancellationToken);

impl Token for ReadToken {
    type Id = RequestId;

    fn id(&self) -> RequestId {
        self.0.request_id()
    }
}

impl ReadToken {
    fn cancel(&self) {
        Request::cancel_sent_request(&self.0);
    }
}

impl ReadPath {
    /// Creates the window timer of `device`'s read path.
    fn create(device: &Device) -> NtResult<Self> {
        Ok(Self {
            completer: SerialCompleter::default(),
            window_timer: Timer::create(&TimerConfig::new_non_periodic(
                device,
                window_timer_fired,
            ))?,
            started: Instant::now(),
        })
    }

    fn now(&self) -> Time {
        Time::from_elapsed(self.started.elapsed())
    }

    /// Starts the window timer to fire at `deadline`, and at least 1 ms from now.
    /// One timer serves every gesture (ADR 7, "Gestures").
    fn arm_window_timer(&self, deadline: Time) {
        let due = deadline.saturating_since(self.now());
        // A due time of 0 is not relative, so the timer waits at least 1 ms.
        self.window_timer.start(&due.max(Duration::from_millis(1)));
    }
}

/// Returns the guard of `mutex`, ignoring poisoning. With panic = "abort",
/// no holder can poison a lock.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

const FILE_DEVICE_KEYBOARD: u32 = 0x0B;

/// HID_CTL_CODE in hidport.h.
const fn hid_ctl_code(function: u32) -> u32 {
    ctl_code(
        FILE_DEVICE_KEYBOARD,
        function,
        METHOD_NEITHER,
        FILE_ANY_ACCESS,
    )
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
/// filter's UMDF service. It holds the button settings that configure.ps1
/// writes (ADR 6, ADR 7) and the filter's diagnostics (ADR 2).
struct Parameters(RegistryKey);

impl Parameters {
    /// The `REG_MULTI_SZ` value of the bindings, one line each (ADR 7, "Settings").
    const GESTURES: &str = "Gestures";
    /// The DWORD value of the click window, in milliseconds (ADR 7, "The click
    /// window").
    const MULTI_CLICK_MS: &str = "MultiClickMs";

    /// Returns the key, or None when it cannot be opened with `access`.
    fn open(device: &Device, access: RegistryAccessRights) -> Option<Self> {
        device
            .open_registry_key(DeviceInstanceKeyType::DeviceSubkey, access)
            .ok()
            .map(Self)
    }

    /// Returns the DWORD value `name`, or None when the key holds no such DWORD.
    fn get(&self, name: &str) -> Option<u32> {
        self.0
            .query_u32(&UnicodeStringBuf::from_str(name).ok()?)
            .ok()
    }

    /// Sets the DWORD value `name`. A failed write goes unreported, since the
    /// filter writes only diagnostics.
    fn set(&self, name: &str, value: u32) {
        if let Ok(name) = UnicodeStringBuf::from_str(name) {
            let _ = self.0.assign_u32(&name, value);
        }
    }

    /// Returns the button settings, with the default click window when the key
    /// holds no `MultiClickMs` value.
    fn config(&self) -> Config {
        let window = self
            .get(Self::MULTI_CLICK_MS)
            .map_or(Config::WINDOW, |ms| Duration::from_millis(ms.into()));
        Config {
            bindings: self.bindings(),
            window,
        }
    }

    /// Returns the bindings that the `Gestures` value states.
    fn bindings(&self) -> Bindings {
        Bindings::from_lines(self.get_lines(Self::GESTURES).iter().map(String::as_str))
    }

    /// Returns the lines of the `REG_MULTI_SZ` value `name`, or none when the key
    /// holds no such value.
    fn get_lines(&self, name: &str) -> Vec<String> {
        UnicodeStringBuf::from_str(name)
            .ok()
            .and_then(|name| self.0.query_multi_string(&name).ok())
            .unwrap_or_default()
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
    /// Records the read in the registry for diagnosis (ADR 2).
    fn record(&self, parameters: &Parameters) {
        parameters.set("XiaoYiRcOriginalLength", self.original_length.into());
        parameters.set("XiaoYiRcReadStatus", dword((&self.reply).into()));
        // The trimmed length is 0 when the read failed or was short.
        let (read_bytes, trimmed_length) = match &self.reply {
            Ok(DescriptorReply::Short { bytes }) => (saturating_dword(*bytes), 0),
            Ok(DescriptorReply::Untrimmed) => {
                (self.original_length.into(), self.original_length.into())
            }
            Ok(DescriptorReply::Trimmed(trimmed)) => {
                (self.original_length.into(), saturating_dword(trimmed.len()))
            }
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
    Driver::create(
        driver_object,
        registry_path,
        DriverConfig::new(evt_device_add),
    )?;
    Ok(())
}

fn evt_device_add(device_init: &mut DeviceInit) -> NtResult<()> {
    device_init.set_filter();
    let device = Device::create(device_init, None)?;
    FilterContext::attach(
        device,
        FilterContext {
            descriptor: Mutex::new(None),
            reads: ReadPath::create(device)?,
        },
    )?;
    IoQueue::create(device, &make_queue_config())?;
    Ok(())
}

/// Returns a queue config with only a device-control callback, since
/// mshidumdf delivers HID IOCTLs as regular device control (ADR 2, task Tpkpmd).
fn make_queue_config() -> IoQueueConfig {
    let mut queue_config = IoQueueConfig::new(IoQueueDispatchType::Parallel {
        presented_requests_limit: None,
    });
    queue_config.default_queue = true;
    queue_config.evt_io_device_control = Some(evt_io_device_control);
    queue_config
}

fn evt_io_device_control(
    queue: &IoQueue,
    request: Request,
    _output_length: usize,
    _input_length: usize,
    code: u32,
) {
    let filter = Filter::new(queue.get_device());
    match Ioctl::parse(code) {
        Ioctl::GetDeviceDescriptor => filter.handle_get_device_descriptor(request),
        Ioctl::GetReportDescriptor => filter.handle_get_report_descriptor(request),
        Ioctl::ReadReport => filter.handle_read_report(request),
        Ioctl::Other => filter.forward_and_forget(request),
    }
}

struct Filter<'a> {
    device: &'a Device,
    target: &'a IoTarget,
    context: &'a FilterContext,
}

impl<'a> Filter<'a> {
    fn new(device: &'a Device) -> Self {
        Self {
            device,
            target: device.get_io_target(),
            context: FilterContext::get(device),
        }
    }

    /// Sends the request to the driver below unchanged. Completes the request
    /// with the send's status when the send fails.
    fn forward_and_forget(&self, mut request: Request) {
        request.format_using_current_type();
        if let Err(request) = request.send_and_forget(self.target) {
            complete_unsent(request);
        }
    }

    /// Takes hidclass's read, which goes up with the next queued report or down
    /// to wait for the remote's next one (ADR 7, "The read path").
    fn handle_read_report(&self, request: Request) {
        self.run_reads(|state| state.park(request));
    }

    /// Completes the request with the driver below's reply, and holds the trimmed
    /// report descriptor whose length the reply now states (ADR 2). When nothing
    /// is trimmed, the filter holds none and the reply goes up as it came.
    /// Records the reply's status and the read for diagnosis.
    fn handle_get_device_descriptor(&self, mut request: Request) {
        request.format_using_current_type();
        let io_status = IoStatusBlock {
            status: request.send_synchronously(self.target),
            information: request.get_information(),
        };

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
                hid.trim(&report)
                    .map_or(DescriptorReply::Untrimmed, |trimmed| {
                        DescriptorReply::Trimmed(trimmed.to_vec())
                    })
            }
        });
        DescriptorRead {
            original_length,
            reply,
        }
    }

    /// Completes the request with the held trimmed report descriptor, without
    /// passing it down (ADR 2). Passes the request down unchanged while none is held.
    fn handle_get_report_descriptor(&self, request: Request) {
        let held = self.context.descriptor().clone();
        let Some(descriptor) = held else {
            return self.forward_and_forget(request);
        };
        complete_with_output(request, &descriptor);
    }

    /// Runs the read path with [`SerialCompleter::run`].
    fn run_reads<U>(&self, record: impl FnOnce(&mut Reads) -> U) -> U {
        self.context
            .reads
            .completer
            .run(record, |action| self.perform(action))
    }

    fn perform(&self, action: ReadAction) {
        match action {
            Action::Complete(request, report) => complete_with_output(request, report.as_bytes()),
            Action::Send(request) => self.send_read(request),
            Action::Cancel(token) => token.cancel(),
            Action::Arm(deadline) => self.context.reads.arm_window_timer(deadline),
        }
    }

    /// Sends hidclass's read down, with [`read_report_complete`] to take what
    /// comes back. Completes the request with the error when the send fails.
    fn send_read(&self, mut request: Request) {
        request.format_using_current_type();
        if let Err(error) = request.set_completion_routine(read_report_complete) {
            return request.complete(error.into());
        }
        let id = request.id();
        let token = ReadToken(request.get_sent_request_cancellation_token());
        // Listed before the send, which can complete before it returns.
        self.run_reads(|state| state.sending(token));
        match request.send_asynchronously(self.target) {
            Ok(()) => self.run_reads(|state| state.sent(id)),
            Err(request) => {
                self.run_reads(|state| state.forget(id));
                complete_unsent(request);
            }
        }
    }

    /// Returns what the driver below completed the read with, and for a report,
    /// the settings read now (ADR 7, "The read path").
    fn returned(&self, request: &mut Request, io_status: &IoStatusBlock) -> Returned {
        if !io_status.status.is_success() {
            return if io_status.status.code() == status_codes::STATUS_CANCELLED {
                Returned::Cancelled
            } else {
                Returned::Failed
            };
        }
        match completed_output(request, io_status).and_then(|buf| InputReport::parse(buf)) {
            Some(report) => Returned::Report {
                report,
                config: self.read_config(),
            },
            None => Returned::Other,
        }
    }

    /// Returns what the registry says the buttons send now, or the default when
    /// the key cannot be opened.
    fn read_config(&self) -> Config {
        Parameters::open(self.device, RegistryAccessRights::Read)
            .map_or_else(Config::default, |p| p.config())
    }
}

/// Completes a request that the driver below did not take, with the status of
/// the failed send.
fn complete_unsent(request: Request) {
    let status = request.get_status();
    request.complete(status);
}

/// What a read brought back from below.
enum Returned {
    /// One of the remote's input reports, with the settings read for it.
    Report { report: InputReport, config: Config },
    /// The read was cancelled, by a flush or by hidclass.
    Cancelled,
    /// A reply that is not the remote's input report, which goes up as it came.
    Other,
    /// An error, which goes up as it came.
    Failed,
}

/// Takes back a read from below.
fn read_report_complete(mut request: Request, target: &IoTarget) {
    let filter = Filter::new(target.get_device());
    let reads = &filter.context.reads;
    let id = request.id();
    // Under UMDF, get_completion_params reports status 0 and length 0 (task Tpkpmd).
    let io_status = IoStatusBlock {
        status: request.get_status(),
        information: request.get_information(),
    };
    let returned = filter.returned(&mut request, &io_status);
    let up_as_came = filter.run_reads(|state| match returned {
        Returned::Report { report, config } => {
            state.came_back(id, request, reads.now(), &report, &config);
            None
        }
        Returned::Cancelled => state.cancelled(id, request),
        Returned::Other => {
            state.forget(id);
            Some(request)
        }
        Returned::Failed => {
            state.failed(id);
            Some(request)
        }
    });
    if let Some(request) = up_as_came {
        request.complete_with_information(io_status.status, io_status.information);
    }
}

/// Queues the taps of the click windows that have ended, and carries them up.
fn window_timer_fired(timer: &Timer) {
    let filter = Filter::new(timer.get_device());
    let config = filter.read_config();
    let reads = &filter.context.reads;
    filter.run_reads(|state| state.expire(reads.now(), &config));
}

/// Completes the request with `bytes` as its output, or with the error when its
/// output buffer cannot take them.
fn complete_with_output(mut request: Request, bytes: &[u8]) {
    match request
        .retrieve_output_memory()
        .and_then(|memory| memory.copy_from_buffer(0, bytes))
    {
        Ok(()) => request
            .complete_with_information(NtStatus::from(status_codes::STATUS_SUCCESS), bytes.len()),
        Err(error) => request.complete(error.into()),
    }
}

/// Returns the reply in the output buffer of a request that the driver below
/// completed with `io_status`. Changes to it change the reply.
/// Returns None when the request failed.
fn completed_output<'a>(
    request: &'a mut Request,
    io_status: &IoStatusBlock,
) -> Option<&'a mut [u8]> {
    if !io_status.status.is_success() {
        return None;
    }
    request
        .retrieve_output_memory()
        .ok()?
        .get_buffer_mut()
        .get_mut(..io_status.information)
}
