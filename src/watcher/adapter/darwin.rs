// import order:

// module
//  types
//  constants
//  functions

#[allow(dead_code)]
pub mod sys {
    pub use core::ffi::*;
    use core::ptr::null_mut;
    pub const NULLPTR: *mut c_void = null_mut();
    pub mod os {
        pub mod darwin {
            pub mod dispatch {
                use super::super::super::{c_char, c_int, c_uint, c_void, NULLPTR};
                pub const DISPATCH_QUEUE_SERIAL: *mut c_void = NULLPTR;
                pub const QOS_CLASS_USER_INTERACTIVE: c_uint = 0x21;
                pub const QOS_CLASS_USER_INITIATED: c_uint = 0x19;
                pub const QOS_CLASS_DEFAULT: c_uint = 0x15;
                pub const QOS_CLASS_UTILITY: c_uint = 0x11;
                pub const QOS_CLASS_BACKGROUND: c_uint = 0x09;
                pub const QOS_CLASS_UNSPECIFIED: c_uint = 0x00;
                pub const QUEUE_PRIORITY_HIGH: i32 = -10;
                pub type DispatchQueueAttr = *mut c_void;
                pub type DispatchObject = *mut c_void;
                #[link(name = "CoreFoundation", kind = "framework")]
                #[link(name = "CoreServices", kind = "framework")]
                extern "C" {
                    fn dispatch_queue_attr_make_with_qos_class(
                        dispatch_queue_attr: DispatchQueueAttr,
                        qos_class: c_uint,
                        relative_priority: c_int,
                    ) -> DispatchQueueAttr;
                    fn dispatch_queue_create(
                        label: *const c_char,
                        dispatch_queue_attr: DispatchQueueAttr,
                    ) -> DispatchQueueAttr;
                    fn dispatch_release(dispatch_object: DispatchObject);
                }
            }
            pub mod cf {
                use super::super::super::{c_char, c_int, c_long, c_uint, c_void};
                use super::dispatch::*;
                pub const ENC_UTF8: u32 = 0x08000100;
                pub type CFArrayRetainCallBack =
                    extern "C" fn(allocator: *const c_void, value: *const c_void) -> *const c_void;
                pub type CFArrayReleaseCallBack =
                    extern "C" fn(allocator: *const c_void, value: *const c_void);
                pub type CFArrayCopyDescriptionCallBack =
                    extern "C" fn(value: *const c_void) -> *mut c_void;
                pub type CFArrayEqualCallBack =
                    extern "C" fn(value1: *const c_void, value2: *const c_void) -> u8;
                pub type MaybeCFAllocatorRetainCallBack =
                    Option<extern "C" fn(*const c_void) -> *const c_void>;
                pub type MaybeCFAllocatorReleaseCallBack = Option<extern "C" fn(*const c_void)>;
                pub type MaybeCFAllocatorCopyDescriptionCallBack =
                    Option<extern "C" fn(*const c_void) -> *const *mut c_void>;
                #[repr(C)]
                pub struct CFArrayCallBacks {
                    pub version: isize,
                    pub retain: CFArrayRetainCallBack,
                    pub release: CFArrayReleaseCallBack,
                    pub copy_description: CFArrayCopyDescriptionCallBack,
                    pub equal: CFArrayEqualCallBack,
                }
                #[link(name = "CoreFoundation", kind = "framework")]
                #[link(name = "CoreServices", kind = "framework")]
                extern "C" {
                    pub static kCFTypeArrayCallBacks: CFArrayCallBacks;
                    pub fn dispatch_queue_attr_make_with_qos_class(
                        dispatch_queue_attr: DispatchQueueAttr,
                        qos_class: c_uint,
                        relative_priority: c_int,
                    ) -> DispatchQueueAttr;
                    pub fn dispatch_queue_create(
                        label: *const c_char,
                        dispatch_queue_attr: DispatchQueueAttr,
                    ) -> DispatchQueueAttr;
                    pub fn dispatch_release(dispatch_object: DispatchObject);
                    pub fn CFDictionaryGetValue(
                        container: *const c_void,
                        key: *const c_void,
                    ) -> *const c_void;
                    pub fn CFArrayGetValueAtIndex(array: *mut c_void, index: isize) -> *mut c_void;
                    pub fn CFStringGetCStringPtr(
                        str_ref: *mut c_void,
                        string_encoding: u32,
                    ) -> *const c_char;
                    pub fn CFArrayCreate(
                        cf_allocator_ref: *mut c_void,
                        values: *const *const c_void,
                        num_values: c_long,
                        cf_array_of_callbacks: *const CFArrayCallBacks,
                    ) -> *mut c_void;
                    pub fn CFStringCreateWithCString(
                        cf_allocator_ref: *const c_void,
                        cstr: *const c_char,
                        string_encoding: u32,
                    ) -> *mut c_void;
                }
            }
            pub mod fs_events {
                use super::super::super::{c_uint, c_void};
                use super::cf::*;
                pub const CREATE: c_uint = 0x00000100;
                pub const DESTROY: c_uint = 0x00000200;
                pub const MODIFY: c_uint = 0x00001000;
                pub const RENAME: c_uint = 0x00000800;
                pub const FILE: c_uint = 0x00010000;
                pub const DIR: c_uint = 0x00020000;
                pub const SYM_LINK: c_uint = 0x00040000;
                pub const HARD_LINK: c_uint = 0x00100000 | 0x00200000;
                pub const FILE_EVENTS: u32 = 0x00000010;
                pub const USE_EXTENDED_DATA: u32 = 0x00000040;
                pub const USE_CF_TYPES: u32 = 0x00000001;
                pub const SINCE_NOW: u64 = 0xFFFFFFFFFFFFFFFF;
                #[repr(C)]
                pub struct FSEventStreamContext {
                    pub version: isize,
                    pub info: *mut c_void,
                    pub retain: MaybeCFAllocatorRetainCallBack,
                    pub release: MaybeCFAllocatorReleaseCallBack,
                    pub copy_description: MaybeCFAllocatorCopyDescriptionCallBack,
                }
                pub type FSEventStreamCallback = extern "C" fn(
                    fs_event_stream_const_ref: *const c_void,
                    client_callback_info: *mut c_void,
                    num_events: usize,
                    event_paths: *mut c_void,
                    event_flags: *const c_uint,
                    event_ids: *const u64,
                );
                #[link(name = "CoreFoundation", kind = "framework")]
                #[link(name = "CoreServices", kind = "framework")]
                extern "C" {
                    pub fn FSEventStreamStart(stream_ref: *mut c_void) -> u8;
                    pub fn FSEventStreamStop(stream_ref: *mut c_void);
                    pub fn FSEventStreamInvalidate(fs_event_stream_ref: *mut c_void);
                    pub fn FSEventStreamRelease(fs_event_stream_ref: *mut c_void);
                    pub fn FSEventStreamRetain(fs_event_stream_ref: *mut c_void);
                    pub fn FSEventStreamSetDispatchQueue(
                        fs_event_stream_ref: *mut c_void,
                        dipatch_queue: *mut c_void,
                    );
                    pub fn FSEventStreamCreate(
                        cf_allocator_ref: *const c_void,
                        fs_event_sream_callback_func_ptr: FSEventStreamCallback,
                        fs_event_sream_context: *const FSEventStreamContext,
                        cf_array_ref_of_paths_to_watch: *const c_void,
                        since_when: u64,
                        latency: f64,
                        flags: c_uint,
                    ) -> *mut c_void;
                }
            }
        }
    }
}

use crate::watcher::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver as SyncReceiver, Sender as SyncSender, TryRecvError::Empty};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sys::os::darwin::cf::*;
use sys::os::darwin::dispatch::*;
use sys::os::darwin::fs_events::*;
use sys::*;

// pub type Callback = dyn FnMut(Event) + Send + Sync + 'static;
// pub type Callback = fn(Event);

#[repr(C)]
struct ArgPtr<'a> {
    event_tx: &'a SyncSender<Event>,
    // callback: &'a Callback,
    seen_created_paths: &'a mut HashSet<String>,
}

impl From<ArgPtr<'_>> for *mut c_void {
    fn from(arg_ptr: ArgPtr) -> *mut c_void {
        Box::into_raw(Box::new(arg_ptr)) as *mut c_void
    }
}

impl From<&mut ArgPtr<'_>> for *mut c_void {
    fn from(arg_ptr: &mut ArgPtr) -> *mut c_void {
        (unsafe { &mut *(arg_ptr as *mut ArgPtr) }) as *mut ArgPtr as *mut c_void
    }
}

type StreamResources = (*mut c_void, DispatchObject);

fn event_stream_open(
    path: String,
    func_ptr: FSEventStreamCallback,
    arg_ptr: &mut ArgPtr,
) -> Option<StreamResources> {
    let mut path_with_null_term = path;
    path_with_null_term.push('\0');

    let bytes = path_with_null_term.as_bytes();

    match CStr::from_bytes_with_nul(bytes) {
        Ok(path_cstrptr_wrapped) => {
            let path_cstr = path_cstrptr_wrapped.as_ptr();
            let func_ptr_context = FSEventStreamContext {
                version: 0,
                info: arg_ptr.into(),
                retain: None,
                release: None,
                copy_description: None,
            };

            let path_ptrptr = unsafe { CFStringCreateWithCString(NULLPTR, path_cstr, ENC_UTF8) };

            let path_array = unsafe {
                CFArrayCreate(
                    NULLPTR,
                    &(path_ptrptr as *const c_void),
                    1, // An array of one element
                    &kCFTypeArrayCallBacks,
                )
            };

            let stream = unsafe {
                FSEventStreamCreate(
                    NULLPTR,
                    func_ptr,
                    &func_ptr_context,
                    path_array as *mut c_void,
                    SINCE_NOW,
                    1_f64, // seconds
                    FILE_EVENTS | USE_EXTENDED_DATA | USE_CF_TYPES,
                )
            };

            // @todo random
            let queue_label: *const c_char = CStr::from_bytes_with_nul(b"queue_label\0")
                .unwrap()
                .as_ptr();

            let queue = unsafe {
                dispatch_queue_create(
                    queue_label,
                    dispatch_queue_attr_make_with_qos_class(
                        DISPATCH_QUEUE_SERIAL,
                        QOS_CLASS_USER_INITIATED,
                        QUEUE_PRIORITY_HIGH,
                    ),
                )
            };

            unsafe {
                FSEventStreamSetDispatchQueue(stream, queue);
                FSEventStreamStart(stream);
            };

            Some((stream, queue as DispatchObject))
        }

        Err(_) => None,
    }
}

fn event_stream_close(resources: StreamResources) -> bool {
    // if let Some((stream, queue)) = resources {
    let (stream, queue) = resources;
    if !stream.is_null() {
        unsafe {
            FSEventStreamStop(stream);
            FSEventStreamInvalidate(stream);
            FSEventStreamRelease(stream);
        }
        if queue as *mut c_void != NULLPTR {
            unsafe { dispatch_release(queue) };
            true
        } else {
            false
        }
    } else {
        false
    }
    // } else {
    //     false
    // }
}

// Gross... and unavoidable.
fn path_from_event_at(paths: *mut c_void, index: isize) -> Option<String> {
    unsafe {
        const KEY_NAME: *const c_char = b"path\0".as_ptr() as *const c_char;
        let dict = CFArrayGetValueAtIndex(paths, index);
        let key = CFStringCreateWithCString(NULLPTR, KEY_NAME, ENC_UTF8);
        let path_from_dict = CFDictionaryGetValue(dict, key) as *mut c_void;
        let path_cstr = CFStringGetCStringPtr(path_from_dict, ENC_UTF8);

        if path_cstr != NULLPTR as *const i8 {
            Some(CStr::from_ptr(path_cstr).to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

const fn kind_from(flag: u32) -> Kind {
    if flag & FILE != 0 {
        Kind::File
    } else if flag & DIR != 0 {
        Kind::Dir
    } else if flag & SYM_LINK != 0 {
        Kind::SymLink
    } else if flag & HARD_LINK != 0 {
        Kind::HardLink
    } else {
        Kind::Other
    }
}

const fn what_from(flag: u32) -> What {
    if flag == CREATE {
        What::Create
    } else if flag == DESTROY {
        What::Destroy
    } else if flag == MODIFY {
        What::Modify
    } else if flag == RENAME {
        What::Rename
    } else {
        What::Other
    }
}

fn now() -> Duration {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since_epoch) => since_epoch,
        Err(_) => Duration::from_nanos(0),
    }
}

macro_rules! any_null {
    ($tok:expr) => ($tok.is_null())
    ;
    ($tok:expr, $($toks:expr),+) => ($tok.is_null() || any_null!($($toks),+))
}

extern "C" fn event_recv(
    _stream: *const c_void,
    context: *mut c_void,
    recv_count: usize,
    recv_paths: *mut c_void,
    recv_flags: *const u32,
    _event_ids: *const u64,
) {
    if !any_null!(context, recv_paths, recv_flags) {
        let arg_ptr = unsafe { &mut *(context as *mut ArgPtr) };

        let (event_tx, scatmap) = (&mut arg_ptr.event_tx, &mut arg_ptr.seen_created_paths);

        for i in 0..recv_count as isize {
            if let Some(path_str) = path_from_event_at(recv_paths, i) {
                let pathbuf = PathBuf::from(&path_str);
                let flags = unsafe { *recv_flags.offset(i).cast::<c_uint>() };
                let inflags = |cmp: u32| flags & cmp != 0;

                macro_rules! send_if_have_flag {
                    ($flag:expr) => {
                        if inflags($flag) {
                            event_tx
                                .send(Event {
                                    path: pathbuf.clone().into(),
                                    what: what_from($flag),
                                    kind: kind_from(flags),
                                    when: now(),
                                })
                                .unwrap_or_default();
                        }
                    };
                }

                let updated_if_needed: bool = match scatmap.get(&path_str) {
                    None if inflags(CREATE) => scatmap.insert(path_str.clone()),
                    Some(_) if inflags(DESTROY) => scatmap.remove(&path_str),
                    _ => !(inflags(CREATE) || inflags(DESTROY)),
                };

                if updated_if_needed {
                    send_if_have_flag!(CREATE);
                    send_if_have_flag!(DESTROY);
                    send_if_have_flag!(MODIFY);
                    send_if_have_flag!(RENAME);
                }
            }
        }
    }
}

// pub fn watch(path: String, callback: Box<Callback>, rx: Receiver<bool>) -> bool {
pub fn open(path: String, event_tx: SyncSender<Event>, ctl_rx: SyncReceiver<bool>) -> bool {
    const DELAY: Duration = Duration::from_millis(16);

    let is_living = || match ctl_rx.try_recv() {
        Err(Empty) => true,
        Ok(false) => false,
        Ok(true) => true,
        Err(_) => false,
    };

    let mut seen_created_paths = HashSet::<String>::new();

    let mut arg_ptr = ArgPtr {
        event_tx: &event_tx,
        seen_created_paths: &mut seen_created_paths,
    };

    let stream_resources = event_stream_open(path, event_recv, &mut arg_ptr);

    match stream_resources {
        Some(stream_resources) => {
            loop {
                if is_living() {
                    sleep(DELAY);
                } else {
                    break;
                }
            }
            event_stream_close(stream_resources)
        }

        None => false,
    }
}
