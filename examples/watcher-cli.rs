use clap::Parser;
use tokio::io::stdin;
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::task;
use tokio_stream::StreamExt;
use wtr::watcher;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct WatcherCliArgs {
    #[arg(long)]
    path: String,
    #[arg(long)]
    filter_path: Option<Vec<String>>,
    #[arg(long)]
    filter_what: Option<Vec<wtr::watcher::What>>,
    #[arg(long)]
    filter_kind: Option<Vec<wtr::watcher::Kind>>,
    #[arg(long)]
    exec: Option<String>,
}

fn escape(s: String) -> String {
    s.replace("\"", "\\\"").replace("'", "\\'")
}

fn have_filtered_result(
    filter_path: &Option<Vec<String>>,
    filter_what: &Option<Vec<wtr::watcher::What>>,
    filter_kind: &Option<Vec<wtr::watcher::Kind>>,
    event: &watcher::Event,
) -> bool {
    filter_path
        .as_ref()
        .map(|filter_path| filter_path.contains(&event.path.to_str().unwrap().to_string()))
        .and_then(|b| if b { Some(b) } else { None })
        .or_else(|| {
            filter_kind
                .as_ref()
                .map(|filter_kind| filter_kind.contains(&event.kind))
                .and_then(|b| if b { Some(b) } else { None })
        })
        .or_else(|| {
            filter_what
                .as_ref()
                .map(|filter_what| filter_what.contains(&event.what))
                .and_then(|b| if b { Some(b) } else { None })
        })
        .unwrap_or(true)
}

async fn any_input() -> bool {
    stdin().read(&mut [0u8]).await.is_ok()
}

async fn on_watch_event(args: WatcherCliArgs, mut bc_rx: tokio::sync::broadcast::Receiver<()>) {
    let mut watcher = watcher::watch(args.path);

    bc_rx.resubscribe();

    loop {
        match bc_rx.try_recv() {
            Ok(_) => {
                watcher.stop();
            }
            Err(TryRecvError::Closed) => {
                watcher.stop();
            }
            _ => {}
        }
        if let Some(event) = watcher.next().await {
            // println!("alaalskdjfnalslalala{}", event);
            if have_filtered_result(
                &args.filter_path,
                &args.filter_what,
                &args.filter_kind,
                &event,
            ) {
                if let Some(exec) = &args.exec {
                    let mut s = exec
                        .to_string()
                        .replace("{event}", &escape(event.to_string()))
                        .replace("{path}", &escape(event.path.to_string_lossy().into_owned()))
                        .replace("{what}", &event.what.to_string())
                        .replace("{kind}", &event.kind.to_string())
                        .replace("{when}", &event.when.as_nanos().to_string());
                    s.push('\0');
                    task::spawn_blocking(move || unsafe { libc::system(s.as_ptr() as *const i8) });
                } else {
                    println!("{}", event);
                }
            }
        }
    }
}

fn main() {
    Runtime::new().unwrap().block_on(async {
        let args = WatcherCliArgs::parse();

        let (bc_tx, bc_rx) = tokio::sync::broadcast::channel(1);

        let watching = task::spawn(on_watch_event(args, bc_rx));

        task::spawn(any_input())
            .await
            .map(|_| {
                let _sent = bc_tx.send(());
                watching.abort();
            })
            .unwrap();
    });
}
