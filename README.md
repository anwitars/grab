# grab

`grab` is a high-performance, declarative stream processor for delimited text data.

It is designed to replace fragile shell pipelines (`awk`, `cut`, `sed`) with a structured approach for data extraction and manipulation. Instead of relying on complex, column-based syntax, `grab` allows you to define your data schema upfront: turning messy, brittle pipelines into readable, maintainable, and verifiable data flows.

## Key Features
- **High Performance:** Process ~17.1M fields/sec (often limited only by system pipe throughput).
- **Safety First:** Strict UTF-8 validation and schema enforcement by default.
- **JQ's Best Friend:** Transform messy delimited text into structured JSON ingress for `jq`.
- **Zero Dependencies:** Single static binary (~800KB). No libc requirements (musl).

## Quick Start

To create JSON objects from a CSV file, you can use the following command:

```bash
# users.csv:
# 1,John,Doe,555-1234,555-5678,London,UK
# 2,Jane,Smith,555-8765,555-4321,New York,USA

grab --mapping id,_,last,phones:2,_:g --json < users.csv

# Output:
# {"id":"1","last":"Doe","phones":["555-1234","555-5678"]}
# {"id":"2","last":"Smith","phones":["555-8765","555-4321"]}
```

Or see processes consuming more than 5% memory:

```bash
ps aux | ./grab --delimiter whitespace --mapping _,pid,_,mem,_:6,command:gj --json --skip 1 | jq -r 'select(.mem | tonumber > 5)'
```

## The UNIX Philosophy

`grab` is built to be a first-class citizen in the UNIX ecosystem. It adheres strictly to the principles of modularity and composability:
- **Everything is a stream**: `grab` reads from `stdin` and writes to `stdout`.
- **Composable by design**: Because it operates on streams, `grab` integrates seamlessly into existing pipelines. It works perfectly between your text sources and downstream processors like `jq` or `grep`.
- **Single responsibility**: It does one thing—transforming delimited text data—and does it well. It avoids "feature bloat" by focusing on high speed, type-safe processing.
- **Transparent failure**: `grab` communicates errors via `stderr` and uses standard exit codes. If a pipeline breaks, you know exactly where and why without digging through opaque error messages.

## Why `grab` vs. shell tools?

| Feature | Conventional Shell Tools | `grab` |
|---------|-------------------------|--------|
| **Logic** | Cryptic column indexing (e.g., `$1`) | Readable, named field mapping (e.g., `name`) |
| **Error Handling** | Silent failures or cryptic errors | Strict validation (opt-out available) with clear error messages |
| **Complexity** | Exponential regex/string logic | Declarative schema definition with built-in transformations |

## Mapping Syntax

| Syntax | Action |
|--------|--------|
| `name` | Maps the next input column to field `name` |
| `_:N` | Skips the next `N` input columns |
| `phones:N` | Maps the next `N` input columns to an array field `phones` |
| `data:g` | Maps the rest of the input columns to an array field `data` |
| `command:gj` | Maps the rest of the input columns into a single field `command` by joining them with spaces |

## Pipeline Integration

`grab` excels at preparing data for specialized JSON tools. Instead of writing complex `jq` logic to handle raw strings, use `grab` to create a clean schema first:

```bash
# Extract IDs and Countries, then use jq to filter and format
grab -m "id,_:5,country,_:g" -d ',' --json < users.csv | jq -r 'select(.country == "UK") | .id'
```

## Performance

### Benchmark

Benchmarks were done as follows:

- **Machine**: Lenovo Thinkpad E15 Gen 2
- **Dataset**: 2 million rows of CSV data with 12 column (~350MB, ~24 million fields)

#### All columns

Even processing 24 million fields while validating the schema, ensuring UTF-8 correctness, and handling errors, `grab` achieves a throughput of 7.6 million fields per second.

```bash
hyperfine --warmup 3 --runs 5 "./grab --mapping index,customer_id,first_name,last_name,company,city,country,phones:2,email,subscription_date,website --skip 1 --json < .demo/2mil.csv > /dev/null"

# Results
# Time (mean ± σ):      2.677 s ±  0.012 s    [User: 2.631 s, System: 0.046 s]
# Range (min … max):    2.662 s …  2.691 s    5 runs
# Throughput: 8.9 million fields/s
```

#### Filtering and taking a subset

When we actually start using `grab` as intended, mapping only the fields we care about and skipping the rest, the performance improves significantly. In this case, we achieve a throughput of 12.8 million fields per second (including skipped ones).

```bash
hyperfine --warmup 3 --runs 5 "./grab --mapping _:2,first_name,last_name,_:3,phones:2,email,_:g --skip 1 --json < .demo/2mil.csv > /dev/null"

# Results
# Time (mean ± σ):      1.397 s ±  0.014 s    [User: 1.357 s, System: 0.040 s]
# Range (min … max):    1.381 s …  1.412 s    5 runs
# Throughput: 17.1 million fields/s
```

#### Note

While profiling, a significant portion of the execution time is spent on system calls and kernel-space I/O. `grab` often operates at the theoretical limit of the system pipe.

### TL;DR

| Task | Fields/Sec | Time |
|------|------------|------|
| All columns with full schema validation | 8.9 million | 2.68s |
| Partial map + greedy skip | **17.1 million** | **1.4s** |

## Installation

### Binaries

Precompiled binaries for Linux are available on the releases page.

### Cargo

You can also install `grab` using Cargo:

```
cargo install grab-cli
```

### Source

To build from source, clone the repository and run:

```
cargo build --release
```

## Contributing

As of now, `grab` is in early development and not yet accepting contributions. However, if you're interested in contributing or have ideas for features, please reach out to me directly.