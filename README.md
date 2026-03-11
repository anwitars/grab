# grab

`grab` is a high-performance, declarative stream processor for delimited text data.

It is designed to replace fragile shell pipelines (`awk`, `cut`, `sed`) with a structured approach for data extraction and manipulation. Instead of relying on complex, column-based syntax, `grab` allows you to define your data schema upfront: turning messy, brittle pipelines into readable, maintainable, and verifiable data flows.

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

## Pipeline Integration

Filter for UK users and extract their IDs

```bash
grep ",UK" users.csv | grab --mapping id,_:g --json | jq -r '.[].id'
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
# Time (mean ± σ):      3.155 s ±  0.031 s    [User: 3.115 s, System: 0.038 s]
# Range (min … max):    3.127 s …  3.196 s    5 runs
# Throughput: 7.6 million fields/s
```

#### Filtering and taking a subset

When we actually start using `grab` as intended, mapping only the fields we care about and skipping the rest, the performance improves significantly. In this case, we achieve a throughput of 12.8 million fields per second (including skipped ones).

```bash
hyperfine --warmup 3 --runs 5 "./grab --mapping _:2,first_name,last_name,_:3,phones:2,email,_:g --skip 1 --json < .demo/2mil.csv > /dev/null"

# Results
# Time (mean ± σ):      1.864 s ±  0.010 s    [User: 1.835 s, System: 0.029 s]
# Range (min … max):    1.852 s …  1.878 s    5 runs
# Throughput: 12.8 million fields/s
```

#### Note

While profiling, a significant portion of the execution time is spent on system calls and kernel-space I/O. `grab` often operates at the theoretical limit of the system pipe.

### TL;DR

| Task | Fields/Sec | Time |
|------|------------|------|
| All columns with full schema validation | 7.6 million | 3.15s |
| Partial map + greedy skip | **12.8 million** | **1.86s** |

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