use crate::watcher::*;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::time::Duration;
use std::cmp::Ordering;
use std::sync::mpsc::channel as sync_channel;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;
use std::sync::mpsc::TryRecvError;
use tokio::task::spawn;
use tokio::task::spawn_blocking;
use tokio::time::sleep;
use tokio_stream::Stream;

#[derive(PartialEq)]
enum State {
    Open,
    Closed,
}

pub struct EventStream {
    state: State,
    worker: tokio::task::JoinHandle<bool>,
    waker: Option<tokio::task::JoinHandle<()>>,
    delay: Delay,
    ctl_tx: SyncSender<bool>,
    event_rx: SyncReceiver<Event>,
}

impl EventStream {
    pub fn stop(&mut self) -> bool {
        match self.state {
            State::Open => {
                self.state = State::Closed;
                let ret = self.ctl_tx.send(false).is_ok();
                if !self.worker.is_finished() {
                    self.worker.abort();
                }
                ret
            }
            State::Closed => true,
        }
    }
}

struct Delay {
    idx: usize,
}

impl Delay {
    const fn ms(d: u64) -> Duration {
        Duration::from_millis(d)
    }

    const PROGRESSION: &[Duration] = &[
        // 16 x 16 ms = 256 ms
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        Delay::ms(16),
        // 8 x 32 ms = 256 ms
        Delay::ms(32),
        Delay::ms(32),
        Delay::ms(32),
        Delay::ms(32),
        Delay::ms(32),
        Delay::ms(32),
        // 8 x 64 ms = 512 ms
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        Delay::ms(64),
        // 4 x 128 = 512 ms
        Delay::ms(128),
        Delay::ms(128),
        Delay::ms(128),
        Delay::ms(128),
        // 2 x 256 ms = 512 ms
        Delay::ms(256),
        Delay::ms(256),
        // 1 x 512 ms = 512 ms
        Delay::ms(512),
        // We don't grow beyond the last delay
    ];

    pub fn new() -> Delay {
        Delay { idx: 0 }
    }

    // pub fn as_duration(&self) -> &Duration {
    //     Delay::PROGRESSION.get(self.idx).unwrap()
    // }

    pub fn to_duration(&self) -> Duration {
        *Delay::PROGRESSION.get(self.idx).unwrap()
    }

    pub fn initial_delay(&self) -> Delay {
        Delay { idx: 0 }
    }

    pub fn reset(&mut self) {
        *self = self.initial_delay();
    }

    pub fn next(&self) -> Delay {
        let clamp_next_idx = match self.idx.cmp(&(Delay::PROGRESSION.len() - 1)) {
            Ordering::Less => self.idx + 1,
            _ => self.idx,
        };

        Delay {
            idx: clamp_next_idx,
        }
    }
}

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let is_chan_ok = |e: TryRecvError| matches!(e, TryRecvError::Empty);

        if self.state == State::Open {
            // println!(
            //     "idx: {} , delay: {} ms",
            //     self.delay.idx,
            //     self.delay.as_duration().as_millis()
            // );
            match self.event_rx.try_recv() {
                /*  We have an event ready now. Send it. */
                Ok(event) => {
                    self.delay.reset();
                    Poll::Ready(Some(event))
                }

                Err(e) => {
                    /*  Nothing received just yet. */
                    if is_chan_ok(e) {
                        /*  The channel is alive.
                         *  Progress our `delay.next()`
                         *  and schedule a wake for ourselves. */
                        self.waker
                            .take()
                            .map(|waker| waker.is_finished())
                            .unwrap_or(true)
                            .then(|| {
                                self.delay = self.delay.next();
                                self.waker = Some(spawn(wake_after(
                                    cx.waker().clone(),
                                    self.delay.to_duration(),
                                )))
                            });

                        Poll::Pending
                    } else {
                        /*  Channel was closed... Or something bad. */
                        Poll::Ready(None)
                    }
                }
            }
        } else {
            /*  We are dead. */
            Poll::Ready(None)
        }
    }
}

async fn wake_after(waker: std::task::Waker, delay: Duration) {
    sleep(delay).await;
    waker.wake_by_ref();
}

pub fn watch(path: String) -> EventStream {
    use State::Open;

    let canonical_path: String = path.clone();
    // std::path::PathBuf::from(&path)
    // .canonicalize()
    // .unwrap()
    // .to_string_lossy()
    // .into_owned();

    let (ctl_tx, ctl_rx) = sync_channel::<bool>();

    let (event_tx, event_rx) = sync_channel::<Event>();

    EventStream {
        state: Open,
        waker: None,
        worker: spawn_blocking(move || adapter::open(canonical_path, event_tx, ctl_rx)),
        delay: Delay::new(),
        ctl_tx,
        event_rx,
    }
}
