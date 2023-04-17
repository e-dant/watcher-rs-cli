mod fanotify;
mod inotify;

use crate::watcher::*;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;

pub fn open(path: String, event_tx: SyncSender<Event>, ctl_rx: SyncReceiver<bool>) -> bool {
    let is_user_root = unsafe { libc::geteuid() } == 0;

    if is_user_root {
        // println!("fanotify");
        fanotify::watch(path, event_tx, ctl_rx)
    } else {
        inotify::watch(path, event_tx, ctl_rx)
    }

    // macro_rules! cond{
    //     ($($pred:expr => $body:expr,),+ $default:expr) => {
    //         $(if ($pred) { $body } else)+ { $default }
    //     };
    // }
    // macro_rules! fanotify {
    //     () => {
    //         fanotify::watch(path, event_tx, ctl_rx)
    //     };
    // }
    // macro_rules! inotify {
    //     () => {
    //         inotify::watch(path, event_tx, ctl_rx)
    //     };
    // }
    // let is_user_root = unsafe { libc::geteuid() } == 0;
    // cond!(is_user_root => fanotify!(), inotify!())
}
