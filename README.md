# Algolia Index Monitor

Algolia Index Monitor is a simple command-line tool for monitoring changes in the size of an Algolia index. If the index size changes, it fetches relevant logs and prints updates about modifications or deletions.

# Install

Use prebuild binary in releases or build one yourself with
```shell
cargo build --release
```

# Usage
```text
Algolia index size monitor

Usage: algolia-monitor [OPTIONS] <APP_ID> <KEY> <INDEX_NAME>

Arguments:
  <APP_ID>      Application ID
  <KEY>         Algolia API key
  <INDEX_NAME>  Name of the index to monitor

Options:
  -e, --expected-records <EXPECTED_RECORDS>  [default: 0]
  -d, --delay <DELAY>                        [default: 30]
      --delta <DELTA>                        [default: 1000]
  -h, --help                                 Print help
  -V, --version                              Print version
```

