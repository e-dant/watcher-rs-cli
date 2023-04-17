use crate::watcher::*;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::ops::Index;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;

#[allow(dead_code)]
pub mod sys {
    pub mod os {
        pub mod linux {
            use core::mem::size_of;

            pub use libc::fanotify_event_metadata;
            pub use libc::fanotify_init;
            pub use libc::fanotify_mark;

            pub use libc::FANOTIFY_METADATA_VERSION;
            pub use libc::FAN_MARK_REMOVE;
            pub use libc::FAN_NOFD;
            pub use libc::FAN_Q_OVERFLOW;

            pub use libc::O_CLOEXEC;
            pub use libc::O_NONBLOCK;
            pub use libc::O_PATH;
            pub use libc::O_RDONLY;

            pub const AT_FDCWD: i32 = -100;

            pub const FAN_CLASS_NOTIF: u32 = 0x00000000;
            pub const FAN_CLASS_CONTENT: u32 = 0x00000004;
            pub const FAN_CLASS_PRE_CONTENT: u32 = 0x00000008;
            pub const FAN_REPORT_DIR_FID: u32 = 0x00000400;
            pub const FAN_REPORT_NAME: u32 = 0x00000800;
            pub const FAN_REPORT_DFID_NAME: u32 = FAN_REPORT_DIR_FID | FAN_REPORT_NAME;
            pub const FAN_UNLIMITED_QUEUE: u32 = 0x00000010;
            pub const FAN_UNLIMITED_MARKS: u32 = 0x00000020;

            pub const FAN_MARK_ADD: u32 = 0x00000001;
            pub const FAN_ONDIR: u64 = 0x40000000;
            pub const FAN_CREATE: u64 = 0x00000100;
            pub const FAN_DELETE: u64 = 0x00000200;
            pub const FAN_MODIFY: u64 = 0x00000002;
            pub const FAN_MOVED_TO: u64 = 0x00000080;
            pub const FAN_MOVED_FROM: u64 = 0x00000040;
            pub const FAN_MOVE: u64 = FAN_MOVED_FROM | FAN_MOVED_TO;
            pub const FAN_DELETE_SELF: u64 = 0x00000400;
            pub const FAN_MOVE_SELF: u64 = 0x00000800;
            pub const FAN_EVENT_METADATA_LEN: usize = size_of::<fanotify_event_metadata>();
            pub const FAN_EVENT_INFO_TYPE_DFID_NAME: usize = 2;

            //  struct file_handle {
            //      // Size of f_handle [in, out]
            //      unsigned int handle_bytes;
            //      // Handle type [out]
            //      int handle_type;
            //      // File identifier (sized by caller) [out]
            //      unsigned char f_handle[0];
            //  };

            #[repr(C)]
            #[allow(non_camel_case_types)]
            pub struct file_handle {
                pub handle_bytes: u32,
                pub handle_type: i32,
                pub f_handle: [u8; 0],
            }

            //  struct __kernel_fsid_t {
            //      int	val[2];
            //  };

            #[repr(C)]
            #[allow(non_camel_case_types, dead_code)]
            pub struct __kernel_fsid_t {
                pub val: [i32; 2],
            }

            // struct fanotify_event_info_header {
            //     __u8 info_type;
            //     __u8 pad;
            //     __u16 len;
            // };

            #[repr(C)]
            #[allow(non_camel_case_types, dead_code)]
            pub struct fanotify_event_info_header {
                pub info_type: u8,
                pub pad: u8,
                pub len: u16,
            }

            // struct fanotify_event_info_fid {
            //     struct fanotify_event_info_header hdr;
            //     __kernel_fsid_t fsid;
            //     unsigned char file_handle[0];
            // };

            #[repr(C)]
            #[allow(non_camel_case_types, dead_code)]
            pub struct fanotify_event_info_fid {
                pub hdr: fanotify_event_info_header,
                pub fsid: __kernel_fsid_t,
                // [u8; 0],
                // handle: *mut file_handle,
                pub handle: [u8; 0],
            }
        }
    }
}

const EVENT_BUF_LEN: usize = 4096;
const DELAY_MS: i32 = 16;
const EVENT_WAIT_QUEUE_MAX: i32 = 64;

type MarkSet = std::collections::HashSet<i32>;

struct SystemResources {
    valid: bool,
    watch_fd: i32,
    event_fd: i32,
    mark_set: MarkSet,
}

fn now() -> std::time::Duration {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since_epoch) => since_epoch,
        Err(_) => std::time::Duration::from_nanos(0),
    }
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn strerrno() -> String {
    unsafe { core::ffi::CStr::from_ptr(libc::strerror(errno())) }
        .to_string_lossy()
        .into_owned()
}

fn markwalk_recursive(mut mark_set: &mut MarkSet, watch_fd: i32, topdir: &Path) {
    use std::os::unix::fs::MetadataExt;

    const DIR_Q_RSRV_COUNT: usize = 4096 * 8;

    // let start_time = std::time::SystemTime::now();

    mark_sys(&topdir, watch_fd, &mut mark_set);

    let mut inode_set = HashSet::<u64>::new();

    let mut dir_queue = VecDeque::<PathBuf>::from([topdir.to_path_buf()]);

    dir_queue.reserve(DIR_Q_RSRV_COUNT);

    // println!("cap of dir queue: {}", dir_queue.capacity());

    'ol: loop {
        if let Some(nexttop) = dir_queue.pop_front() {
            if let Ok(mut entries) = fs::read_dir(nexttop) {
                for maybe_dirent in entries.by_ref() {
                    if let Ok(dirent) = maybe_dirent {
                        if let Ok(meta) = fs::metadata(dirent.path()) {
                            let ino = meta.ino();
                            if !inode_set.contains(&ino) {
                                inode_set.insert(ino);
                                if mark_sys(&dirent.path(), watch_fd, &mut mark_set) {
                                    dir_queue.push_back(dirent.path());
                                }
                            }
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

    // println!(
    //     "done in: {} ms ({} s)",
    //     start_time.elapsed().unwrap().as_millis(),
    //     start_time.elapsed().unwrap().as_secs()
    // );
    // println!("cap of dir queue: {}", dir_queue.capacity());
}

fn make_mark_set(watch_fd: RawFd, base_path: &Path) -> MarkSet {
    const MARK_SET_RSRV_COUNT: usize = 256;

    let mut mark_set = MarkSet::new();

    mark_set.reserve(MARK_SET_RSRV_COUNT);

    markwalk_recursive(&mut mark_set, watch_fd, base_path);

    mark_set
}

fn make_system_resources(base_path: &Path) -> SystemResources {
    use sys::os::linux::*;

    const FAN_INIT_FLAGS: u32 =
        FAN_CLASS_NOTIF | FAN_REPORT_DFID_NAME | FAN_UNLIMITED_QUEUE | FAN_UNLIMITED_MARKS;
    const FAN_INIT_OPT_FLAGS: u32 = (O_RDONLY | O_NONBLOCK | O_CLOEXEC) as u32;

    let do_error = |msg: &str, watch_fd: i32, event_fd: i32| -> SystemResources {
        println!("{} : {}", msg, strerrno());
        SystemResources {
            valid: false,
            watch_fd,
            event_fd,
            mark_set: MarkSet::new(),
        }
    };

    // println!(
    //     "init :: flags: {} / opts: {}",
    //     FAN_INIT_FLAGS, FAN_INIT_OPT_FLAGS
    // );
    let watch_fd = unsafe { fanotify_init(FAN_INIT_FLAGS, FAN_INIT_OPT_FLAGS) };

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
                SystemResources {
                    valid: true,
                    watch_fd,
                    event_fd,
                    mark_set: make_mark_set(watch_fd, base_path),
                }
            } else {
                do_error("e/sys/epoll_ctl", watch_fd, event_fd)
            }
        } else {
            do_error("e/sys/epoll_create", watch_fd, event_fd)
        }
    } else {
        do_error("e/sys/fanotify_init", watch_fd, -1)
    }
}

fn close_system_resources(sr: &mut SystemResources) -> bool {
    unsafe { libc::close(sr.watch_fd) + libc::close(sr.event_fd) == 0 }
}

// fn print_fanotify_event_metadata(mtd: *mut fanotify_event_metadata) {
//     println!("fanotify_event_metadata @ {:p}:", mtd);
//     println!(" event_len: {}", unsafe { *mtd }.event_len);
//     println!(" vers: {}", unsafe { *mtd }.vers);
//     println!(" reserved: {}", unsafe { *mtd }.reserved);
//     println!(" metadata_len: {}", unsafe { *mtd }.metadata_len);
//     println!(" mask: {}", unsafe { *mtd }.mask);
//     println!(" fd: {}", unsafe { *mtd }.fd);
//     println!(" pid: {}", unsafe { *mtd }.pid);
// }

fn promote(mtd: *const sys::os::linux::fanotify_event_metadata) -> Option<Event> {
    // (bool, PathBuf, What, Kind) {
    use libc::close;
    use libc::readlink;
    use libc::snprintf;
    use libc::SYS_open_by_handle_at;
    use std::mem::size_of;
    use std::mem::size_of_val;
    use sys::os::linux::*;

    let path_imbue = |path_accum: &mut [u8],
                      dir_fid_info: *const fanotify_event_info_fid,
                      dir_fh: *const file_handle,
                      dir_name_len: usize| unsafe {
        const CWD_CSTR: *const i8 = b".\0".as_ptr() as *const i8;
        const FMT_CSTR: *const i8 = b"/%s\0".as_ptr() as *const i8;

        let name_info: *const i8 = dir_fid_info.add(1) as *const i8;
        let file_name: *const i8 = name_info
            .add(size_of::<file_handle>())
            .add(size_of_val(&(*dir_fh).f_handle))
            .add(size_of_val(&(*dir_fh).handle_bytes))
            .add(size_of_val(&(*dir_fh).handle_type));

        // println!("dir_fid_info @ {:p}", dir_fid_info);
        // println!("name_info @ {:p}", name_info);
        // println!("file_name @ {:p}", file_name);
        // println!(
        //     "file_name (str) : {:?}",
        //     core::ffi::CStr::from_ptr(file_name)
        // );

        if !file_name.is_null() && (libc::strcmp(file_name, CWD_CSTR) != 0) {
            let start_ptr = (path_accum.as_ptr() as usize + dir_name_len) as *mut i8;
            let n: usize = 4096 - dir_name_len;
            libc::snprintf(start_ptr, n, FMT_CSTR, file_name);
        }
    };

    let dir_fid_info: *const fanotify_event_info_fid =
        unsafe { mtd.add(1) as *const fanotify_event_info_fid };

    let dir_fh: *mut file_handle = unsafe { dir_fid_info.add(1) as *mut file_handle };

    let what = unsafe {
        match (*mtd).mask & FAN_CREATE != 0 {
            true => What::Create,
            false => match (*mtd).mask & FAN_DELETE != 0 {
                true => What::Destroy,
                false => What::Other,
            },
        }
    };

    let kind = match unsafe { *mtd }.mask & FAN_ONDIR != 0 {
        true => Kind::Dir,
        false => Kind::File,
    };

    /* We can get a path name, so get that and use it */
    let mut path_buf: [u8; EVENT_BUF_LEN] = [0; EVENT_BUF_LEN];
    let fd = unsafe {
        libc::syscall(
            SYS_open_by_handle_at,
            AT_FDCWD,
            dir_fh,
            O_RDONLY | O_CLOEXEC | O_PATH | O_NONBLOCK,
        )
    };

    if fd > 0 {
        const FS_PROC_PATH_BUF_LEN: usize = 128;
        const FS_PROC_PATH_FMT_CSTR: *const i8 = b"/proc/self/fd/%d\0".as_ptr() as *const i8;

        let mut fs_proc_path_buf: [u8; FS_PROC_PATH_BUF_LEN] = [0; FS_PROC_PATH_BUF_LEN];

        let cp_n = unsafe {
            snprintf(
                fs_proc_path_buf.as_mut_ptr() as *mut i8,
                FS_PROC_PATH_BUF_LEN,
                FS_PROC_PATH_FMT_CSTR,
                fd,
            )
        };

        // -1 for the null byte
        let dirname_len: usize = unsafe {
            readlink(
                fs_proc_path_buf.as_mut_ptr() as *mut i8,
                path_buf.as_mut_ptr() as *mut i8,
                EVENT_BUF_LEN - 1,
            )
        }
        .try_into()
        .unwrap_or(0);

        unsafe { close(fd as i32) };

        if cp_n > 0 && dirname_len > 0 {
            //  Put the directory name in the path accumulator.
            //  Passing `dirname_len` has the effect of putting
            //  the event's filename in the path buffer as well.

            // Next line not needed unless we use `maybe_uninit`
            // instead of zero initializing the buf
            // path_buf[dirname_len] = '\0';
            path_imbue(&mut path_buf, dir_fid_info, dir_fh, dirname_len);

            let path_str = unsafe { std::str::from_utf8_unchecked(&path_buf) };
            let path = Path::new(&path_str);

            // println!("have path: {}", ret.to_str().unwrap());

            Some(Event {
                path: path.into(),
                what,
                kind,
                when: now(),
            })
        } else {
            // println!("empty path, strerrno: {}", strerrno());
            // return (false, Path::new("").to_path_buf(), what, kind);
            None
        }
    } else {
        path_imbue(&mut path_buf, dir_fid_info, dir_fh, 0);

        let path_str = unsafe { std::str::from_utf8_unchecked(&path_buf) };
        let path = Path::new(&path_str);

        Some(Event {
            path: path.into(),
            what,
            kind,
            when: now(),
        })

        // return (true, Path::new(&path_str).to_path_buf(), what, kind);
    }
}

fn mark_sys(full_path: &Path, watch_fd: i32, mark_set: &mut MarkSet) -> bool {
    use sys::os::linux::*;
    if full_path.is_dir() {
        const FLAGS: u32 = FAN_MARK_ADD;
        const MASK: u64 = FAN_ONDIR
            | FAN_CREATE
            | FAN_MODIFY
            | FAN_DELETE
            | FAN_MOVE
            | FAN_DELETE_SELF
            | FAN_MOVE_SELF;

        let full_path_cstr = full_path.as_os_str().as_ref() as *const std::ffi::OsStr as *const i8;

        let wd = unsafe { fanotify_mark(watch_fd, FLAGS, MASK, AT_FDCWD, full_path_cstr) };
        if wd >= 0 {
            mark_set.insert(wd);
            true
        } else {
            // println!(
            //     "oops, while marking, bad fanotify mark call :: path: {} :: errno: {}",
            //     unsafe { core::ffi::CStr::from_ptr(full_path_cstr as *const i8).to_string_lossy() },
            //     strerrno()
            // );
            false
        }
    } else {
        false
    }
}

fn unmark_sys(full_path: &Path, watch_fd: i32, mark_set: &mut MarkSet) -> bool {
    use sys::os::linux::*;

    if full_path.is_dir() {
        const FLAGS: u32 = FAN_MARK_REMOVE;
        const MASK: u64 = FAN_ONDIR
            | FAN_CREATE
            | FAN_MODIFY
            | FAN_DELETE
            | FAN_MOVE
            | FAN_DELETE_SELF
            | FAN_MOVE_SELF;

        let full_path_cstr = full_path.as_os_str().to_str().unwrap_or("\0").as_ptr() as *const i8;

        let wd = unsafe { fanotify_mark(watch_fd, FLAGS, MASK, AT_FDCWD, full_path_cstr) };

        if wd >= 0 {
            let _ = mark_set.remove(&wd);
            true
        } else {
            // println!(
            //     "oops, while unmarking, bad fanotify mark call :: path: {} :: errno: {}",
            //     unsafe { core::ffi::CStr::from_ptr(full_path_cstr).to_string_lossy() },
            //     strerrno()
            // );
            false
        }
    } else {
        false
    }
}

fn check_and_update<'a>(
    maybe_event: &'a Option<Event>,
    sr: &'a mut SystemResources,
) -> &'a Option<Event> {
    // let (valid, path, what, kind) = r;

    if let &Some(event) = &maybe_event {
        if event.kind == Kind::Dir {
            if event.what == What::Create {
                // println!(
                //     "trying to mark kind:dir/what:create for path {}",
                //     (*path).to_string_lossy()
                // );
                mark_sys(&event.path, sr.watch_fd, &mut sr.mark_set);
            } else if event.what == What::Destroy {
                // println!(
                //     "trying to unmark kind:dir/what:create for path {}",
                //     (*path).to_string_lossy()
                // );
                unmark_sys(&event.path, sr.watch_fd, &mut sr.mark_set);
            }
        }

        maybe_event
    } else {
        &None
    }
}

fn recv(sr: &mut SystemResources, _base_path: &Path, event_tx: SyncSender<Event>) -> bool {
    use core::ffi::*;
    use libc::read;
    use libc::EAGAIN;
    use sys::os::linux::*;

    const BLANK_FANOTIFY_EVENT_METADATA: fanotify_event_metadata = fanotify_event_metadata {
        event_len: 0,
        vers: 0,
        reserved: 0,
        metadata_len: 0,
        mask: 0,
        fd: 0,
        pid: 0,
    };

    enum State {
        Ok,
        None,
        Err,
    }

    /* Read some events. */
    let mut event_buf: [fanotify_event_metadata; EVENT_BUF_LEN] =
        [BLANK_FANOTIFY_EVENT_METADATA; EVENT_BUF_LEN];

    let mut event_read_len = unsafe {
        read(
            sr.watch_fd,
            event_buf.as_mut_ptr() as *mut c_void,
            EVENT_BUF_LEN,
        )
    };

    let event_read_state = match event_read_len.cmp(&0) {
        Ordering::Greater => State::Ok,
        Ordering::Equal => State::None,
        Ordering::Less => {
            if errno() == EAGAIN {
                State::None
            } else {
                State::Err
            }
        }
    };

    match event_read_state {
        State::Ok => {
            let mut mtd = event_buf.as_mut_ptr();

            let readable = |mtd_ptr: *const fanotify_event_metadata, buf_read_len: isize| -> bool {
                const FAN_EVENT_METADATA_LEN_AS_U32: u32 = 24;
                const FAN_EVENT_METADATA_LEN_AS_ISIZE: isize = 24;

                let mtd_dref = unsafe { *mtd_ptr };

                let buf_read_len_ok = buf_read_len >= FAN_EVENT_METADATA_LEN_AS_ISIZE;
                let event_len_large_enough = mtd_dref.event_len >= FAN_EVENT_METADATA_LEN_AS_U32;
                let event_len_fits_in_read_buf =
                    mtd_dref.event_len <= buf_read_len.try_into().unwrap_or(0);
                let event_len_would_align_next_event = mtd_dref.event_len % 0x8 == 0;

                buf_read_len_ok
                    && event_len_large_enough
                    && event_len_fits_in_read_buf
                    && event_len_would_align_next_event
            };

            let metadata_ok = |mtd_ptr: *const fanotify_event_metadata| -> bool {
                let mtd_dref = unsafe { *mtd_ptr };
                let ok_no_fd = mtd_dref.fd == FAN_NOFD;
                let ok_version = mtd_dref.vers == FANOTIFY_METADATA_VERSION;
                let ok_no_overflow = mtd_dref.mask & FAN_Q_OVERFLOW == 0;

                ok_no_fd && ok_version && ok_no_overflow
            };

            let next_event = |mtd_ptr: *mut fanotify_event_metadata,
                              buf_read_len: isize|
             -> (*mut fanotify_event_metadata, isize) {
                let mtd_dref = unsafe { *mtd_ptr };
                let this_event_len = mtd_dref.event_len;
                let next_event_read_len = buf_read_len - this_event_len as isize;
                let next_mtd_ptr =
                    (mtd_ptr as usize + this_event_len as usize) as *mut fanotify_event_metadata;

                (next_mtd_ptr, next_event_read_len)
            };

            while readable(mtd, event_read_len) && metadata_ok(mtd) {
                if let Some(event) = check_and_update(&promote(mtd), sr) {
                    let send_ok = event_tx.send(event.clone());
                    let _ = send_ok.map_err(|e| println!("send err: {}", e));
                }
                (mtd, event_read_len) = next_event(mtd, event_read_len);
            }
            true
        }
        State::None => true,
        State::Err => false,
    }
}

pub fn watch(path_string: String, event_tx: SyncSender<Event>, ctl_rx: SyncReceiver<bool>) -> bool {
    use std::sync::mpsc::TryRecvError::Empty;

    //  While living, with
    //     - A lifetime the user hasn't ended
    //     - A historical map of watch descriptors
    //       to long paths (for event reporting)
    //     - System resources for fanotify and epoll
    //     - An event buffer for events from epoll
    //
    //  Do
    //     - Await filesystem events
    //     - Send errors and events

    let is_living = || match ctl_rx.try_recv() {
        Err(Empty) => true,
        Ok(false) => false,
        Ok(true) => true,
        Err(_) => false,
    };

    let path = Path::new(path_string.as_str());
    let mut sr = make_system_resources(path);
    let mut event_recv_list =
        [libc::epoll_event { events: 0, u64: 0 }; EVENT_WAIT_QUEUE_MAX as usize];
    let event_recv_list_ptr = event_recv_list.as_mut_ptr() as *mut libc::epoll_event;

    if sr.valid {
        while is_living() {
            let event_count = unsafe {
                libc::epoll_wait(
                    sr.event_fd,
                    event_recv_list_ptr,
                    EVENT_WAIT_QUEUE_MAX,
                    DELAY_MS,
                )
            };

            match event_count.cmp(&0) {
                Ordering::Less => {
                    close_system_resources(&mut sr);
                    return false;
                }
                Ordering::Greater => {
                    for n in 0..event_count {
                        let this_event_fd = event_recv_list.index(n as usize).u64;
                        if this_event_fd == sr.watch_fd as u64
                            && !recv(&mut sr, path, event_tx.clone())
                        {
                            close_system_resources(&mut sr);
                            // println!("e/self/event_recv : {}", strerrno());
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
        // println!("e/self/sys_resource : {}", strerrno());
        false
    }
}
