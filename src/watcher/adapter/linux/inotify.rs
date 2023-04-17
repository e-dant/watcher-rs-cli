use crate::watcher::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::ops::Index;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;

#[allow(dead_code)]
pub mod sys {
    pub use core::ffi::*;
    use core::ptr::null_mut;
    pub const NULLPTR: *mut c_void = null_mut();
    pub mod os {
        pub mod linux {
            use super::super::*;
            pub const IN_CREATE: u32 = 0x00000100;
            pub const IN_MODIFY: u32 = 0x00000002;
            pub const IN_DELETE: u32 = 0x00000200;
            pub const IN_ISDIR: u32 = 0x40000000;
            pub const IN_Q_OVERFLOW: u32 = 0x00004000;
            pub const IN_MOVED_FROM: u32 = 0x00000040;
            pub const IN_MOVED_TO: u32 = 0x00000080;
            pub const IN_MOVE: u32 = IN_MOVED_FROM | IN_MOVED_TO;

            extern "C" {
                pub fn inotify_add_watch(fd: c_int, pathname: *const c_char, mask: u32) -> c_int;
            }
        }
    }
}

type DirMap = HashMap<i32, String>;

fn make_dir_map(base_path: &Path, watch_fd: RawFd) -> DirMap {
    use sys::os::linux::*;

    // Follow symlinks, ignore paths which we don't have permissions for.
    const DIR_MAP_RESERVE_COUNT: usize = 256;
    const IN_WATCH_OPT: u32 = IN_CREATE | IN_MODIFY | IN_DELETE | IN_MOVED_FROM | IN_Q_OVERFLOW;

    let mut pm = DirMap::new();
    pm.reserve(DIR_MAP_RESERVE_COUNT);

    let mut do_mark = |dir: &Path| {
        if dir.is_dir() {
            let mut dir_buf: Vec<u8> = dir.to_str().unwrap().as_bytes().to_vec();
            dir_buf.push(b'\0');

            let wd =
                unsafe { inotify_add_watch(watch_fd, dir_buf.as_ptr() as *const i8, IN_WATCH_OPT) };
            if wd >= 0 {
                // or just > ?
                let dir_string = dir.to_str().unwrap().to_string();
                pm.insert(wd, dir_string);
                true
            } else {
                false
            }
        } else {
            false
        }
    };

    let mut markwalk_recursive = |topdir: PathBuf| {
        do_mark(base_path);
        let mut dirvec = vec![topdir];
        'ol: loop {
            if let Some(nexttop) = dirvec.pop() {
                if let Ok(mut entries) = fs::read_dir(nexttop) {
                    for entry in entries.by_ref() {
                        if let Ok(dir) = entry {
                            if do_mark(&dir.path()) {
                                dirvec.push(dir.path());
                            }
                        } else {
                            break 'ol;
                        }
                    }
                } else {
                    break 'ol;
                }
            } else {
                break 'ol;
            }
        }
    };

    markwalk_recursive(base_path.to_path_buf());

    pm
}

/*  @brief wtr/watcher/<d>/adapter/linux/inotify/<a>/fns/system_unfold
Produces a `sys_resource_type` with the file descriptors from
`inotify_init` and `epoll_create`. Invokes `callback` on errors. */
struct SystemResource {
    valid: bool,
    watch_fd: i32,
    event_fd: i32,
    // event_conf: libc::epoll_event,
}

fn make_system_resources() -> SystemResource {
    let do_error = |msg: &str, watch_fd: i32, event_fd: i32| -> SystemResource {
        println!("{} : {}", msg, strerrno());
        SystemResource {
            valid: false,
            watch_fd,
            event_fd,
            // event_conf: libc::epoll_event {
            //     events: 0,
            //     u64: event_fd as u64,
            // },
        }
    };

    let watch_fd = unsafe { libc::inotify_init() };
    // let watch_fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK) }; // Broken?

    if watch_fd >= 0 {
        let mut event_conf = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: watch_fd as u64,
        };

        let event_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };

        if event_fd >= 0 {
            if unsafe { libc::epoll_ctl(event_fd, libc::EPOLL_CTL_ADD, watch_fd, &mut event_conf) }
                >= 0
            {
                SystemResource {
                    valid: true,
                    watch_fd,
                    event_fd,
                    // event_conf,
                }
            } else {
                do_error("e/sys/epoll_ctl", watch_fd, event_fd)
            }
        } else {
            do_error("e/sys/epoll_create", watch_fd, event_fd)
        }
    } else {
        do_error("e/sys/inotify_init", watch_fd, -1)
    }
}

fn close_system_resources(sr: &mut SystemResource) -> bool {
    unsafe { libc::close(sr.watch_fd) + libc::close(sr.event_fd) == 0 }
}

enum EventRecvState {
    Eventful,
    Eventless,
    Error,
}

fn now() -> std::time::Duration {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since_epoch) => since_epoch,
        Err(_) => std::time::Duration::from_nanos(0),
    }
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct inotify_event {
    pub wd: core::ffi::c_int,
    pub mask: u32,
    pub cookie: u32,
    pub len: u32,
    pub name: [u8; 0],
}

fn recv(watch_fd: i32, pm: &mut DirMap, event_tx: SyncSender<Event>) -> bool {
    use core::ffi::c_void;
    use sys::os::linux::*;

    const EVENT_BUF_LEN: usize = 4096;
    const IN_WATCH_OPT: u32 = IN_CREATE | IN_MODIFY | IN_DELETE | IN_MOVED_FROM | IN_Q_OVERFLOW;

    let mut path_buf: [u8; EVENT_BUF_LEN] = [0; EVENT_BUF_LEN];
    let path_buf_ptr = path_buf.as_mut_ptr();

    // While inotify has events pending, read them.
    // There might be several events from a single read.
    //
    // Three possible states:
    //  - eventful: there are events to read
    //  - eventless: there are no events to read
    //  - error: there was an error reading events
    //
    // The EAGAIN "error" means there is nothing
    // to read. We count that as 'eventless'.
    //
    // Forward events and errors to the user.
    //
    // Return when eventless.

    let default_cached_base_path = String::from("");

    'readloop: loop {
        let read_len = unsafe { libc::read(watch_fd, path_buf_ptr as *mut c_void, EVENT_BUF_LEN) };

        let state = match read_len > 0 {
            true => EventRecvState::Eventful,
            false => match read_len == 0 {
                true => match std::io::Error::last_os_error().raw_os_error() {
                    Some(libc::EAGAIN) => EventRecvState::Eventless,
                    _ => EventRecvState::Error,
                },
                false => EventRecvState::Error,
            },
        };

        let should_continue = match state {
            EventRecvState::Eventful => {
                /* Loop over all events in the buffer. */
                let mut this_event_ptr = path_buf_ptr as *const inotify_event;

                while unsafe {
                    (path_buf_ptr as *const usize).offset_from(this_event_ptr as *const usize)
                } >= 0
                {
                    let this_event = unsafe { &(*this_event_ptr) };

                    if (this_event.mask & IN_Q_OVERFLOW) == 0 {
                        let cached_base_path =
                            pm.get(&this_event.wd).unwrap_or(&default_cached_base_path);
                        let this_event_name_cstr = this_event.name.as_ptr() as *const i8;
                        let name = unsafe { core::ffi::CStr::from_ptr(this_event_name_cstr) };
                        let name_str = name.to_str().unwrap();
                        let mut path_string: String =
                            String::from_str(cached_base_path).unwrap_or_default();
                        path_string.push('/');
                        path_string.push_str(name_str);

                        let kind = match (this_event.mask & IN_ISDIR) != 0 {
                            true => Kind::Dir,
                            false => Kind::File,
                        };

                        let what = match (this_event.mask & IN_CREATE) != 0 {
                            true => What::Create,
                            false => match (this_event.mask & IN_DELETE) != 0 {
                                true => What::Destroy,
                                false => match (this_event.mask & IN_MOVE) != 0 {
                                    true => What::Rename,
                                    false => match (this_event.mask & IN_MODIFY) != 0 {
                                        true => What::Modify,
                                        false => What::Other,
                                    },
                                },
                            },
                        };

                        let when = now();

                        let path = PathBuf::from_str(&path_string).unwrap().into_boxed_path();

                        let _s = event_tx.send(Event {
                            path,
                            what,
                            kind,
                            when,
                        });

                        if kind == Kind::Dir && what == What::Create {
                            let new_wd = unsafe {
                                libc::inotify_add_watch(
                                    watch_fd,
                                    this_event_name_cstr,
                                    IN_WATCH_OPT,
                                )
                            };
                            pm.insert(new_wd, path_string);
                        } else if kind == Kind::Dir && what == What::Destroy {
                            unsafe { libc::inotify_rm_watch(watch_fd, this_event.wd) };
                            let _v = pm.remove(&this_event.wd);
                        }
                    }
                    // else {
                    //     println!("e/sys/overflow : {}", strerrno());
                    // }

                    let next_event_ptr = unsafe { this_event_ptr.add(1) };
                    this_event_ptr = next_event_ptr;
                }
                true
            }

            EventRecvState::Error => false,

            EventRecvState::Eventless => true,
        };

        if !should_continue {
            break 'readloop;
        }
    }

    true
}

fn strerrno() -> String {
    let errno = unsafe { *libc::__errno_location() };
    unsafe { core::ffi::CStr::from_ptr(libc::strerror(errno)) }
        .to_string_lossy()
        .into_owned()
}

pub fn watch(path: String, event_tx: SyncSender<Event>, ctl_rx: SyncReceiver<bool>) -> bool {
    use std::sync::mpsc::TryRecvError::Empty;

    //  While living, with
    //     - A lifetime the user hasn't ended
    //     - A historical map of watch descriptors
    //       to long paths (for event reporting)
    //     - System resources for inotify and epoll
    //     - An event buffer for events from epoll
    //
    //  Do
    //     - Await filesystem events
    //     - Send errors and events

    const EVENT_WAIT_QUEUE_MAX: i32 = 1; // Maximum events before we're awoken

    let is_living = || match ctl_rx.try_recv() {
        Err(Empty) => true,
        Ok(false) => false,
        Ok(true) => true,
        Err(_) => false,
    };

    let pb = PathBuf::from(path);
    let mut sr = make_system_resources();
    let mut pm = make_dir_map(&pb, sr.watch_fd);
    let mut event_recv_list =
        [libc::epoll_event { events: 0, u64: 0 }; EVENT_WAIT_QUEUE_MAX as usize];
    let event_recv_list_ptr = event_recv_list.as_mut_ptr() as *mut libc::epoll_event;

    if sr.valid {
        if !pm.is_empty() {
            while is_living() {
                let event_count = unsafe {
                    libc::epoll_wait(sr.event_fd, event_recv_list_ptr, EVENT_WAIT_QUEUE_MAX, 16)
                };

                match event_count.cmp(&0) {
                    Ordering::Less => {
                        close_system_resources(&mut sr);
                        println!("e/sys/epoll_wait : {}", strerrno());
                        return false;
                    }
                    Ordering::Greater => {
                        for n in 0..event_count {
                            let this_event_fd = event_recv_list.index(n as usize).u64;
                            if this_event_fd == sr.watch_fd as u64
                                && !recv(sr.watch_fd, &mut pm, event_tx.clone())
                            {
                                close_system_resources(&mut sr);
                                println!("e/self/event_recv : {}", strerrno());
                                return false;
                            }
                        }
                    }
                    _ => continue,
                }
            }

            close_system_resources(&mut sr)
        } else {
            close_system_resources(&mut sr);
            println!("e/self/path_map : {}", strerrno());
            false
        }
    } else {
        close_system_resources(&mut sr);
        println!("e/self/sys_resource : {}", strerrno());
        false
    }
}

// if event_count < 0 {
//     close_system_resources(sr);
//     println!("e/sys/epoll_wait : {}", strerrno());
//     return false;
// } else if event_count > 0 {
//     for n in 0..event_count {
//         let this_event_fd = event_recv_list.index(n as usize).u64;
//         if this_event_fd == sr.watch_fd as u64 {
//             if !recv(sr.watch_fd, &mut pm, event_tx.clone()) {
//                 close_system_resources(sr);
//                 println!("e/self/event_recv : {}", strerrno());
//                 return false;
//             }
//         }
//     }
// }
