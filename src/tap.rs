//! Core Audio process tap (macOS 14.4+): listens to the output of specific
//! apps without changing where their audio goes, so no BlackHole/Multi-Output
//! setup is needed and volume keys keep working.
//!
//! Setup: find the app's audio process objects by bundle ID, describe a
//! private stereo-mixdown tap with `CATapDescription`, create the tap, wrap it
//! in a private aggregate device (clocked by the default output device) and
//! read that device with a plain IOProc.

use crate::audio::{Shared, mono, push};
use anyhow::{Context, Result, bail};
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{msg_send, sel};
use objc2_foundation::{NSArray, NSNumber, NSString, NSUUID};
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr;

type AudioObjectID = u32;
type OSStatus = i32;
type IoProcId = *mut c_void;
type IoProc = extern "C" fn(
    AudioObjectID,
    *const c_void,
    *const AudioBufferList,
    *const c_void,
    *mut AudioBufferList,
    *const c_void,
    *mut c_void,
) -> OSStatus;

#[repr(C)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[repr(C)]
struct AudioBuffer {
    channels: u32,
    byte_size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioBufferList {
    count: u32,
    buffers: [AudioBuffer; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StreamDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        id: AudioObjectID,
        addr: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> OSStatus;
    fn AudioObjectGetPropertyData(
        id: AudioObjectID,
        addr: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> OSStatus;
    fn AudioHardwareCreateProcessTap(desc: *mut AnyObject, out: *mut AudioObjectID) -> OSStatus;
    fn AudioHardwareDestroyProcessTap(id: AudioObjectID) -> OSStatus;
    fn AudioHardwareCreateAggregateDevice(desc: CFDictionaryRef, out: *mut AudioObjectID) -> OSStatus;
    fn AudioHardwareDestroyAggregateDevice(id: AudioObjectID) -> OSStatus;
    fn AudioDeviceCreateIOProcID(
        device: AudioObjectID,
        proc_: IoProc,
        client: *mut c_void,
        out: *mut IoProcId,
    ) -> OSStatus;
    fn AudioDeviceDestroyIOProcID(device: AudioObjectID, id: IoProcId) -> OSStatus;
    fn AudioDeviceStart(device: AudioObjectID, id: IoProcId) -> OSStatus;
    fn AudioDeviceStop(device: AudioObjectID, id: IoProcId) -> OSStatus;
}

const SYSTEM_OBJECT: AudioObjectID = 1;
const FORMAT_FLAG_FLOAT: u32 = 1;

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

fn address(selector: &[u8; 4]) -> PropertyAddress {
    PropertyAddress { selector: fourcc(selector), scope: fourcc(b"glob"), element: 0 }
}

fn check(status: OSStatus, what: &str) -> Result<()> {
    if status != 0 {
        let code = status.to_be_bytes();
        let text = if code.iter().all(|c| c.is_ascii_graphic()) {
            format!(" '{}'", String::from_utf8_lossy(&code))
        } else {
            String::new()
        };
        bail!("{what} failed (OSStatus {status}{text})");
    }
    Ok(())
}

/// Reads a fixed-size property.
fn get<T: Copy>(id: AudioObjectID, selector: &[u8; 4]) -> Result<T> {
    let mut value = MaybeUninit::<T>::zeroed();
    let mut size = size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(id, &address(selector), 0, ptr::null(), &mut size, value.as_mut_ptr().cast())
    };
    check(status, &format!("reading property {}", String::from_utf8_lossy(selector)))?;
    Ok(unsafe { value.assume_init() })
}

fn get_ids(id: AudioObjectID, selector: &[u8; 4]) -> Result<Vec<AudioObjectID>> {
    let addr = address(selector);
    let mut size = 0u32;
    check(
        unsafe { AudioObjectGetPropertyDataSize(id, &addr, 0, ptr::null(), &mut size) },
        "reading property size",
    )?;
    let mut ids = vec![0 as AudioObjectID; size as usize / size_of::<AudioObjectID>()];
    check(
        unsafe { AudioObjectGetPropertyData(id, &addr, 0, ptr::null(), &mut size, ids.as_mut_ptr().cast()) },
        "reading id list",
    )?;
    ids.truncate(size as usize / size_of::<AudioObjectID>());
    Ok(ids)
}

fn get_string(id: AudioObjectID, selector: &[u8; 4]) -> Result<String> {
    let s: CFStringRef = get(id, selector)?;
    if s.is_null() {
        return Ok(String::new());
    }
    Ok(unsafe { CFString::wrap_under_create_rule(s) }.to_string())
}

pub struct AudioProcess {
    object: AudioObjectID,
    pub pid: i32,
    pub bundle_id: String,
    pub playing: bool,
}

/// Processes currently known to the audio server.
pub fn processes() -> Result<Vec<AudioProcess>> {
    get_ids(SYSTEM_OBJECT, b"prs#")?
        .into_iter()
        .map(|object| {
            Ok(AudioProcess {
                object,
                pid: get(object, b"ppid")?,
                bundle_id: get_string(object, b"pbid").unwrap_or_default(),
                playing: get::<u32>(object, b"piro").unwrap_or(0) != 0,
            })
        })
        .collect()
}

pub fn list_apps() -> Result<()> {
    let mut procs = processes()?;
    procs.sort_by(|a, b| b.playing.cmp(&a.playing).then(a.bundle_id.cmp(&b.bundle_id)));
    println!("Audio processes (▶ = currently playing):");
    for p in procs.iter().filter(|p| !p.bundle_id.is_empty()) {
        println!("{} {:>6}  {}", if p.playing { '▶' } else { ' ' }, p.pid, p.bundle_id);
    }
    Ok(())
}

struct Callback {
    buf: Shared,
}

/// Runs on the Core Audio realtime thread.
extern "C" fn io_proc(
    _device: AudioObjectID,
    _now: *const c_void,
    input: *const AudioBufferList,
    _input_time: *const c_void,
    _output: *mut AudioBufferList,
    _output_time: *const c_void,
    client: *mut c_void,
) -> OSStatus {
    if input.is_null() || client.is_null() {
        return 0;
    }
    let cb = unsafe { &*(client as *const Callback) };
    let list = unsafe { &*input };
    let buffers = unsafe { std::slice::from_raw_parts(list.buffers.as_ptr(), list.count as usize) };
    let samples = |b: &AudioBuffer| -> &[f32] {
        if b.data.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(b.data as *const f32, b.byte_size as usize / 4) }
        }
    };
    match buffers {
        [] => {}
        // Interleaved: one buffer with all channels.
        [b] => push(&cb.buf, mono(samples(b), b.channels.max(1) as usize, |s| s)),
        // Non-interleaved: one buffer per channel.
        many => {
            let chans: Vec<&[f32]> = many.iter().map(samples).collect();
            let frames = chans.iter().map(|c| c.len()).min().unwrap_or(0);
            let n = chans.len() as f32;
            push(&cb.buf, (0..frames).map(|i| chans.iter().map(|c| c[i]).sum::<f32>() / n));
        }
    }
    0
}

/// Live tap; tears everything down on drop.
pub struct ProcessTap {
    tap: AudioObjectID,
    device: AudioObjectID,
    proc_id: IoProcId,
    callback: *mut Callback,
    pub sample_rate: u32,
    pub label: String,
}

impl ProcessTap {
    /// Taps every audio process whose bundle ID contains `app`
    /// (case-insensitive), e.g. "chrome" matches `com.google.Chrome.helper`.
    pub fn start(app: &str, buf: Shared) -> Result<Self> {
        let needle = app.to_lowercase();
        let matches: Vec<AudioProcess> = processes()?
            .into_iter()
            .filter(|p| p.bundle_id.to_lowercase().contains(&needle))
            .collect();
        if matches.is_empty() {
            bail!("no audio process matching '{app}' — is it playing? (see --list-apps)");
        }
        let mut bundles: Vec<&str> = matches.iter().map(|p| p.bundle_id.as_str()).collect();
        bundles.dedup();
        let label = format!("tap: {} ({} proc)", bundles.join(", "), matches.len());

        let mut this = Self {
            tap: 0,
            device: 0,
            proc_id: ptr::null_mut(),
            callback: ptr::null_mut(),
            sample_rate: 0,
            label,
        };

        let numbers: Vec<Retained<NSNumber>> = matches.iter().map(|p| NSNumber::new_u32(p.object)).collect();
        let ids = NSArray::from_retained_slice(&numbers);
        let class = AnyClass::get(c"CATapDescription")
            .context("CATapDescription not available (process taps need macOS 14.4+)")?;
        let desc: Retained<AnyObject> = unsafe {
            let alloc: Allocated<AnyObject> = msg_send![class, alloc];
            let desc: Option<Retained<AnyObject>> = msg_send![alloc, initStereoMixdownOfProcesses: &*ids];
            desc.context("CATapDescription init failed")?
        };
        let tap_uid = unsafe {
            let name = NSString::from_str("soundscene");
            let _: () = msg_send![&*desc, setName: &*name];
            let _: () = msg_send![&*desc, setPrivate: true];
            // macOS 26: re-attach when the app restarts its audio process.
            let restore: bool = msg_send![&*desc, respondsToSelector: sel!(setProcessRestoreEnabled:)];
            if restore {
                let _: () = msg_send![&*desc, setProcessRestoreEnabled: true];
            }
            let uuid: Retained<NSUUID> = msg_send![&*desc, UUID];
            uuid.UUIDString().to_string()
        };
        check(
            unsafe { AudioHardwareCreateProcessTap(Retained::as_ptr(&desc) as *mut AnyObject, &mut this.tap) },
            "creating process tap",
        )?;

        let output: AudioObjectID = get(SYSTEM_OBJECT, b"dOut")?;
        let output_uid = get_string(output, b"uid ")?;
        let key = |k: &str| CFString::new(k);
        let tap_entry = CFDictionary::from_CFType_pairs(&[
            (key("uid"), CFString::new(&tap_uid).as_CFType()),
            (key("drift"), CFBoolean::true_value().as_CFType()),
        ]);
        let sub_device = CFDictionary::from_CFType_pairs(&[(key("uid"), CFString::new(&output_uid).as_CFType())]);
        let aggregate = CFDictionary::from_CFType_pairs(&[
            (key("name"), CFString::new("soundscene tap").as_CFType()),
            (key("uid"), CFString::new(&format!("soundscene-{}", std::process::id())).as_CFType()),
            (key("master"), CFString::new(&output_uid).as_CFType()),
            (key("private"), CFBoolean::true_value().as_CFType()),
            (key("stacked"), CFBoolean::false_value().as_CFType()),
            (key("tapautostart"), CFBoolean::true_value().as_CFType()),
            (key("subdevices"), CFArray::from_CFTypes(&[sub_device]).as_CFType()),
            (key("taps"), CFArray::from_CFTypes(&[tap_entry]).as_CFType()),
        ]);
        check(
            unsafe { AudioHardwareCreateAggregateDevice(aggregate.as_concrete_TypeRef(), &mut this.device) },
            "creating aggregate device",
        )?;

        let format: StreamDescription = get(this.tap, b"tfmt")?;
        if format.format_flags & FORMAT_FLAG_FLOAT == 0 || format.bits_per_channel != 32 {
            bail!("unexpected tap format: flags {:#x}, {} bits", format.format_flags, format.bits_per_channel);
        }
        let rate: f64 = get(this.device, b"nsrt").unwrap_or(format.sample_rate);
        this.sample_rate = rate.round() as u32;

        this.callback = Box::into_raw(Box::new(Callback { buf }));
        check(
            unsafe { AudioDeviceCreateIOProcID(this.device, io_proc, this.callback.cast(), &mut this.proc_id) },
            "creating IOProc",
        )?;
        check(unsafe { AudioDeviceStart(this.device, this.proc_id) }, "starting tap device")?;
        Ok(this)
    }
}

impl Drop for ProcessTap {
    fn drop(&mut self) {
        unsafe {
            if !self.proc_id.is_null() {
                AudioDeviceStop(self.device, self.proc_id);
                AudioDeviceDestroyIOProcID(self.device, self.proc_id);
            }
            if self.device != 0 {
                AudioHardwareDestroyAggregateDevice(self.device);
            }
            if self.tap != 0 {
                AudioHardwareDestroyProcessTap(self.tap);
            }
            if !self.callback.is_null() {
                drop(Box::from_raw(self.callback));
            }
        }
    }
}
