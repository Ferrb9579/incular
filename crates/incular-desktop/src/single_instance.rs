//! Optional single-instance ownership and authenticated local activation forwarding.

use fs4::{FileExt, TryLockError};
use incular_platform::{
    ApplicationActivation, DocumentActivation, GlobalShortcutId, LaunchActivation,
    SingleInstancePolicy, UrlActivation, application_data_local_directory,
};
use interprocess::ConnectWaitMode;
use interprocess::local_socket::{
    ConnectOptions, GenericFilePath, GenericNamespaced, ListenerNonblockingMode, ListenerOptions,
    NameType, Stream, ToFsName, ToNsName, traits::Listener as _,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{HashSet, VecDeque},
    ffi::OsString,
    fmt,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use url::Url;

const PROTOCOL_VERSION: u16 = 1;
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const READY_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_DELAY: Duration = Duration::from_millis(15);
const STREAM_IO_TIMEOUT: Duration = Duration::from_secs(1);
const STREAM_IO_RETRY_DELAY: Duration = Duration::from_millis(2);
const DEDUPLICATION_WINDOW: usize = 1024;

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type SharedWakeCallback = Arc<Mutex<Option<WakeCallback>>>;

#[derive(Debug)]
pub enum SingleInstanceError {
    DataDirectoryUnavailable,
    Io(std::io::Error),
    Protocol(String),
    ForwardingTimedOut,
}

impl fmt::Display for SingleInstanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataDirectoryUnavailable => {
                formatter.write_str("single-instance local data directory is unavailable")
            }
            Self::Io(error) => write!(formatter, "single-instance I/O failed: {error}"),
            Self::Protocol(message) => write!(formatter, "single-instance protocol: {message}"),
            Self::ForwardingTimedOut => formatter.write_str(
                "single-instance primary did not become reachable before the forwarding deadline",
            ),
        }
    }
}

impl std::error::Error for SingleInstanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::DataDirectoryUnavailable | Self::Protocol(_) | Self::ForwardingTimedOut => None,
        }
    }
}

impl From<std::io::Error> for SingleInstanceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[doc(hidden)]
pub enum SingleInstanceRole {
    Primary(SingleInstancePrimary),
    SecondaryForwarded,
}

/// Primary-process ownership. The listener thread only enqueues normalized
/// activations; the desktop event loop drains them on the UI thread.
#[doc(hidden)]
pub struct SingleInstancePrimary {
    lock_file: File,
    endpoint: Endpoint,
    receiver: mpsc::Receiver<ApplicationActivation>,
    wake: SharedWakeCallback,
    shutdown: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl fmt::Debug for SingleInstancePrimary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SingleInstancePrimary")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl SingleInstancePrimary {
    #[doc(hidden)]
    pub fn set_wake(&self, wake: WakeCallback) {
        *self.wake.lock().expect("single-instance wake lock") = Some(wake.clone());
        // Cover the narrow race where an activation arrived between listener
        // startup and Winit proxy installation.
        wake();
    }

    #[doc(hidden)]
    pub fn take_activations(&self) -> Vec<ApplicationActivation> {
        let mut activations = Vec::new();
        while let Ok(activation) = self.receiver.try_recv() {
            activations.push(activation);
        }
        activations
    }
}

impl Drop for SingleInstancePrimary {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = FileExt::unlock(&self.lock_file);
    }
}

#[doc(hidden)]
pub fn acquire_single_instance(
    policy: &SingleInstancePolicy,
    activation: ApplicationActivation,
) -> Result<SingleInstanceRole, SingleInstanceError> {
    let directory = application_data_local_directory(policy.application_id())
        .ok_or(SingleInstanceError::DataDirectoryUnavailable)?
        .join("instance");
    acquire_single_instance_in(policy, activation, &directory)
}

/// Deterministic storage-root seam used by cross-process integration tests.
#[doc(hidden)]
pub fn acquire_single_instance_in(
    policy: &SingleInstancePolicy,
    activation: ApplicationActivation,
    directory: &Path,
) -> Result<SingleInstanceRole, SingleInstanceError> {
    prepare_instance_directory(directory)?;
    let lock_path = directory.join("primary.lock");
    let metadata_path = directory.join("primary.json");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    restrict_file_to_owner(&lock_path)?;
    let message_id = random_bytes::<16>()?;
    let deadline = Instant::now() + READY_TIMEOUT;

    loop {
        match FileExt::try_lock(&lock_file) {
            Ok(()) => return become_primary(policy, directory, &metadata_path, lock_file),
            Err(TryLockError::Error(error)) => return Err(SingleInstanceError::Io(error)),
            Err(TryLockError::WouldBlock) => {}
        }

        let forward_error = match read_metadata(&metadata_path) {
            Ok(metadata)
                if metadata.protocol_version == PROTOCOL_VERSION
                    && metadata.application_id == policy.application_id() =>
            {
                let envelope = ForwardEnvelope {
                    protocol_version: PROTOCOL_VERSION,
                    application_id: policy.application_id().to_owned(),
                    token: metadata.token,
                    message_id,
                    activation: WireActivation::from_activation(&activation),
                };
                match forward(&metadata.endpoint, &envelope) {
                    Ok(()) => return Ok(SingleInstanceRole::SecondaryForwarded),
                    Err(error) => error,
                }
            }
            Ok(_) => SingleInstanceError::Protocol(
                "primary metadata belongs to a different protocol or application".to_owned(),
            ),
            Err(error) => error,
        };

        if Instant::now() >= deadline {
            return match FileExt::try_lock(&lock_file) {
                Ok(()) => become_primary(policy, directory, &metadata_path, lock_file),
                Err(TryLockError::WouldBlock) => Err(forward_error),
                Err(TryLockError::Error(error)) => Err(SingleInstanceError::Io(error)),
            };
        }
        thread::sleep(RETRY_DELAY);
    }
}

fn become_primary(
    policy: &SingleInstancePolicy,
    directory: &Path,
    metadata_path: &Path,
    lock_file: File,
) -> Result<SingleInstanceRole, SingleInstanceError> {
    let token = random_bytes::<32>()?;
    let (endpoint, listener) = create_listener(directory)?;
    let metadata = PrimaryMetadata {
        protocol_version: PROTOCOL_VERSION,
        application_id: policy.application_id().to_owned(),
        endpoint: endpoint.clone(),
        token,
    };
    write_metadata(metadata_path, &metadata)?;

    let application_id = policy.application_id().to_owned();
    let (sender, receiver) = mpsc::channel();
    let wake = Arc::new(Mutex::new(None::<WakeCallback>));
    let thread_wake = wake.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = shutdown.clone();
    let thread = thread::Builder::new()
        .name("incular-single-instance".to_owned())
        .spawn(move || {
            listen_loop(
                listener,
                application_id,
                token,
                sender,
                thread_wake,
                thread_shutdown,
            );
        })
        .map_err(SingleInstanceError::Io)?;

    Ok(SingleInstanceRole::Primary(SingleInstancePrimary {
        lock_file,
        endpoint,
        receiver,
        wake,
        shutdown,
        thread: Some(thread),
    }))
}

fn listen_loop(
    listener: interprocess::local_socket::Listener,
    application_id: String,
    token: [u8; 32],
    sender: mpsc::Sender<ApplicationActivation>,
    wake: SharedWakeCallback,
    shutdown: Arc<AtomicBool>,
) {
    let mut seen = HashSet::<[u8; 16]>::new();
    let mut seen_order = VecDeque::<[u8; 16]>::new();
    loop {
        let mut stream = match listener.accept() {
            Ok(stream) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(RETRY_DELAY);
                continue;
            }
            Err(_) => {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(RETRY_DELAY);
                continue;
            }
        };
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let accepted =
            read_frame_until::<ForwardEnvelope>(&mut stream, Instant::now() + STREAM_IO_TIMEOUT)
                .ok()
                .filter(|envelope| {
                    envelope.protocol_version == PROTOCOL_VERSION
                        && envelope.application_id == application_id
                        && envelope.token == token
                })
                .and_then(|envelope| {
                    if seen.contains(&envelope.message_id) {
                        return Some(true);
                    }
                    let activation = envelope.activation.into_activation().ok()?;
                    if sender.send(activation).is_err() {
                        return Some(false);
                    }
                    seen.insert(envelope.message_id);
                    seen_order.push_back(envelope.message_id);
                    while seen_order.len() > DEDUPLICATION_WINDOW {
                        if let Some(expired) = seen_order.pop_front() {
                            seen.remove(&expired);
                        }
                    }
                    let wake = wake
                        .lock()
                        .expect("single-instance wake lock")
                        .as_ref()
                        .cloned();
                    if let Some(wake) = wake {
                        wake();
                    }
                    Some(true)
                })
                .unwrap_or(false);
        let _ = write_all_until(
            &mut stream,
            &[u8::from(accepted)],
            Instant::now() + STREAM_IO_TIMEOUT,
        );
    }
}

fn forward(endpoint: &Endpoint, envelope: &ForwardEnvelope) -> Result<(), SingleInstanceError> {
    let mut stream = connect_endpoint(endpoint)?;
    let deadline = Instant::now() + STREAM_IO_TIMEOUT;
    write_frame_until(&mut stream, envelope, deadline)?;
    let mut acknowledgement = [0_u8; 1];
    read_exact_until(&mut stream, &mut acknowledgement, deadline)?;
    if acknowledgement[0] == 1 {
        Ok(())
    } else {
        Err(SingleInstanceError::Protocol(
            "primary rejected activation envelope".to_owned(),
        ))
    }
}

fn create_listener(
    directory: &Path,
) -> Result<(Endpoint, interprocess::local_socket::Listener), SingleInstanceError> {
    let nonce = hex(&random_bytes::<16>()?);
    if GenericNamespaced::is_supported() {
        let name = OsString::from(format!("incular-{nonce}"));
        let socket_name = name.clone().to_ns_name::<GenericNamespaced>()?;
        let listener = ListenerOptions::new()
            .name(socket_name)
            .nonblocking(ListenerNonblockingMode::Both)
            .create_sync()?;
        Ok((
            Endpoint::Namespaced(WireOsString::from_os_string(name)),
            listener,
        ))
    } else {
        let path = directory.join(format!("instance-{nonce}.sock"));
        let socket_name = path.clone().to_fs_name::<GenericFilePath>()?;
        let listener = ListenerOptions::new()
            .name(socket_name)
            .nonblocking(ListenerNonblockingMode::Both)
            .create_sync()?;
        Ok((
            Endpoint::Filesystem(WireOsString::from_os_string(path.into_os_string())),
            listener,
        ))
    }
}

fn connect_endpoint(endpoint: &Endpoint) -> Result<Stream, SingleInstanceError> {
    let name = match endpoint {
        Endpoint::Namespaced(name) => name
            .to_os_string()?
            .to_ns_name::<GenericNamespaced>()
            .map_err(SingleInstanceError::Io)?,
        Endpoint::Filesystem(path) => PathBuf::from(path.to_os_string()?)
            .to_fs_name::<GenericFilePath>()
            .map_err(SingleInstanceError::Io)?,
    };
    ConnectOptions::new()
        .name(name)
        .wait_mode(ConnectWaitMode::Timeout(STREAM_IO_TIMEOUT))
        .nonblocking_stream(true)
        .connect_sync()
        .map_err(SingleInstanceError::Io)
}

fn write_metadata(path: &Path, metadata: &PrimaryMetadata) -> Result<(), SingleInstanceError> {
    let bytes = serde_json::to_vec(metadata)
        .map_err(|error| SingleInstanceError::Protocol(error.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    restrict_file_to_owner(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.sync_data()?;
    Ok(())
}

fn prepare_instance_directory(path: &Path) -> Result<(), SingleInstanceError> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn restrict_file_to_owner(path: &Path) -> Result<(), SingleInstanceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn read_metadata(path: &Path) -> Result<PrimaryMetadata, SingleInstanceError> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take((MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
        return Err(SingleInstanceError::Protocol(
            "primary metadata is empty or oversized".to_owned(),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| SingleInstanceError::Protocol(error.to_string()))
}

fn write_frame_until<T: Serialize>(
    stream: &mut Stream,
    value: &T,
    deadline: Instant,
) -> Result<(), SingleInstanceError> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| SingleInstanceError::Protocol(error.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(SingleInstanceError::Protocol(
            "activation payload exceeds the 4 MiB local IPC limit".to_owned(),
        ));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| SingleInstanceError::Protocol("activation payload is too large".to_owned()))?;
    write_all_until(stream, &length.to_le_bytes(), deadline)?;
    write_all_until(stream, &payload, deadline)?;
    Ok(())
}

fn read_frame_until<T: DeserializeOwned>(
    stream: &mut Stream,
    deadline: Instant,
) -> Result<T, SingleInstanceError> {
    let mut length = [0_u8; 4];
    read_exact_until(stream, &mut length, deadline)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(SingleInstanceError::Protocol(
            "activation payload exceeds the 4 MiB local IPC limit".to_owned(),
        ));
    }
    let mut payload = vec![0_u8; length];
    read_exact_until(stream, &mut payload, deadline)?;
    serde_json::from_slice(&payload)
        .map_err(|error| SingleInstanceError::Protocol(error.to_string()))
}

fn read_exact_until(
    stream: &mut Stream,
    mut buffer: &mut [u8],
    deadline: Instant,
) -> Result<(), SingleInstanceError> {
    while !buffer.is_empty() {
        match stream.read(buffer) {
            // Win32 PIPE_NOWAIT reports an empty pipe as a zero-byte read
            // rather than `WouldBlock`. Treat it as temporary unavailability;
            // a disconnected peer is still bounded by the same deadline.
            Ok(0) => wait_for_stream_io(deadline)?,
            Ok(read) => buffer = &mut buffer[read..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                wait_for_stream_io(deadline)?;
            }
            Err(error) => return Err(SingleInstanceError::Io(error)),
        }
    }
    Ok(())
}

fn write_all_until(
    stream: &mut Stream,
    mut buffer: &[u8],
    deadline: Instant,
) -> Result<(), SingleInstanceError> {
    while !buffer.is_empty() {
        match stream.write(buffer) {
            Ok(0) => {
                return Err(SingleInstanceError::Io(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "single-instance local IPC stream stopped accepting data",
                )));
            }
            Ok(written) => buffer = &buffer[written..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                wait_for_stream_io(deadline)?;
            }
            Err(error) => return Err(SingleInstanceError::Io(error)),
        }
    }
    Ok(())
}

fn wait_for_stream_io(deadline: Instant) -> Result<(), SingleInstanceError> {
    if Instant::now() >= deadline {
        return Err(SingleInstanceError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "single-instance local IPC operation timed out",
        )));
    }
    thread::sleep(STREAM_IO_RETRY_DELAY);
    Ok(())
}

fn random_bytes<const N: usize>() -> Result<[u8; N], SingleInstanceError> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes).map_err(|error| {
        SingleInstanceError::Protocol(format!("secure random source unavailable: {error}"))
    })?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[(byte >> 4) as usize] as char);
        value.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    value
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PrimaryMetadata {
    protocol_version: u16,
    application_id: String,
    endpoint: Endpoint,
    token: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Endpoint {
    Namespaced(WireOsString),
    Filesystem(WireOsString),
}

#[derive(Debug, Serialize, Deserialize)]
struct ForwardEnvelope {
    protocol_version: u16,
    application_id: String,
    token: [u8; 32],
    message_id: [u8; 16],
    activation: WireActivation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum WireOsString {
    Unix(Vec<u8>),
    Windows(Vec<u16>),
}

impl WireOsString {
    fn from_os_string(value: OsString) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Self::Unix(value.into_vec())
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Self::Windows(value.encode_wide().collect())
        }
    }

    fn to_os_string(&self) -> Result<OsString, SingleInstanceError> {
        match self {
            Self::Unix(bytes) => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    Ok(OsString::from_vec(bytes.clone()))
                }
                #[cfg(not(unix))]
                {
                    let _ = bytes;
                    Err(SingleInstanceError::Protocol(
                        "received Unix path encoding on a non-Unix host".to_owned(),
                    ))
                }
            }
            Self::Windows(units) => {
                #[cfg(windows)]
                {
                    use std::os::windows::ffi::OsStringExt;
                    Ok(OsString::from_wide(units))
                }
                #[cfg(not(windows))]
                {
                    let _ = units;
                    Err(SingleInstanceError::Protocol(
                        "received Windows path encoding on a non-Windows host".to_owned(),
                    ))
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum WireActivation {
    Launch {
        arguments: Vec<WireOsString>,
        documents: Vec<WireOsString>,
        urls: Vec<String>,
    },
    OpenFiles(Vec<WireOsString>),
    OpenUrls(Vec<String>),
    Reopen,
    GlobalShortcut(u64),
}

impl WireActivation {
    fn from_activation(activation: &ApplicationActivation) -> Self {
        match activation {
            ApplicationActivation::Launch(launch) => Self::Launch {
                arguments: launch
                    .arguments()
                    .iter()
                    .cloned()
                    .map(WireOsString::from_os_string)
                    .collect(),
                documents: launch
                    .documents()
                    .documents()
                    .iter()
                    .map(|document| {
                        WireOsString::from_os_string(document.path().as_os_str().to_os_string())
                    })
                    .collect(),
                urls: launch.urls().iter().map(Url::to_string).collect(),
            },
            ApplicationActivation::OpenFiles(documents) => Self::OpenFiles(
                documents
                    .documents()
                    .iter()
                    .map(|document| {
                        WireOsString::from_os_string(document.path().as_os_str().to_os_string())
                    })
                    .collect(),
            ),
            ApplicationActivation::OpenUrls(urls) => {
                Self::OpenUrls(urls.urls().iter().map(Url::to_string).collect())
            }
            ApplicationActivation::Reopen => Self::Reopen,
            ApplicationActivation::GlobalShortcutInvoked(id) => Self::GlobalShortcut(id.get()),
        }
    }

    fn into_activation(self) -> Result<ApplicationActivation, SingleInstanceError> {
        match self {
            Self::Launch {
                arguments,
                documents,
                urls,
            } => Ok(ApplicationActivation::Launch(
                LaunchActivation::new(
                    arguments
                        .iter()
                        .map(WireOsString::to_os_string)
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .with_documents(DocumentActivation::from_paths(
                    documents
                        .iter()
                        .map(WireOsString::to_os_string)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(PathBuf::from),
                ))
                .with_urls(parse_urls(urls)?),
            )),
            Self::OpenFiles(paths) => Ok(ApplicationActivation::OpenFiles(
                DocumentActivation::from_paths(
                    paths
                        .iter()
                        .map(WireOsString::to_os_string)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(PathBuf::from),
                ),
            )),
            Self::OpenUrls(urls) => Ok(ApplicationActivation::OpenUrls(UrlActivation::new(
                parse_urls(urls)?,
            ))),
            Self::Reopen => Ok(ApplicationActivation::Reopen),
            Self::GlobalShortcut(id) => Ok(ApplicationActivation::GlobalShortcutInvoked(
                GlobalShortcutId::new(id),
            )),
        }
    }
}

fn parse_urls(urls: Vec<String>) -> Result<Vec<Url>, SingleInstanceError> {
    urls.into_iter()
        .map(|url| {
            Url::parse(&url).map_err(|error| {
                SingleInstanceError::Protocol(format!("invalid forwarded URL: {error}"))
            })
        })
        .collect()
}
