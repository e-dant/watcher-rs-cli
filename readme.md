# Watcher, but with a nice CLI and a special kind of plant fungus

```sh
# On linux, best with `sudo` because we get to use fanotify
cargo run -- --path /some/path --exec 'some program --can=handle,arguments --just-fine'
```

```sh
# These {} will be auto-formatted according to the event
cargo run -- --path / --exec 'echo {when} {path} {what} {kind}'
```

```sh
  cargo run -- --help
Usage: watcher-cli [OPTIONS] --path <PATH>

Options:
      --path <PATH>
      --filter-path <FILTER_PATH>
      --filter-what <FILTER_WHAT>
      --filter-kind <FILTER_KIND>
      --exec <EXEC>
  -h, --help                       Print help
  -V, --version                    Print version
```

