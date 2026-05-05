# samesame

A Rust CLI that finds repeated fragments of code across files. Point it at a
directory or a list of files; it reports every region of `--match` or more
consecutive identical lines and where each copy lives.

```bash
cargo install samesame
samesame -d . -g "**/*.rs" -m 8
```

## How it finds duplicate blocks

The pipeline is a hash-and-compare in five steps:

1. **Read and normalize.** Each file is read in parallel via `rayon`. Lines are
   trimmed before hashing (so indentation differences don't break a match), but
   the original line is preserved for verbose output. Binary files (detected by
   null bytes in the first 8KB), symlinks, and empty files are skipped.

2. **Hash every line.** Each line is hashed with `xxh3` (64-bit) into a compact
   `u64`. From this point on, files are vectors of integers.

3. **Roll a window of `min_match` lines.** For every starting offset in every
   file, the tool computes a **Buzhash** over the next `min_match` line hashes.
   Buzhash combines per-line hashes with position-dependent bit rotations
   (`rotate_left`) and XOR. Position dependence matters: permuted lines produce
   a different window hash, so swapped blocks aren't reported as identical.
   Rolling means each window costs O(1) to compute from the previous one — the
   whole pass is linear in total lines.

4. **Group by hash.** All windows are bucketed by their Buzhash key in a single
   `FxHashMap`. Any bucket with two or more entries is a candidate duplicate.
   Pairs sharing the same `(file_a, file_b, offset)` are then walked forward and
   merged into the longest run that stays in lockstep, so a 40-line duplicate
   reports as one 40-line region rather than 33 overlapping 8-line windows.

5. **Consolidate across files.** Pairwise regions covering the same span in
   three or more files are unioned (union-find on `(file, start, end)` keys)
   into a single multi-file `DuplicateGroup`.

The replaced approach used patience diff plus LCS / Hirschberg / longest
increasing subsequence per file pair — a fundamentally O(n²) shape over file
count, plus expensive per-pair work. The rolling-hash design collapses the
detection phase to a single linear pass plus hash-table lookups.

## Speed

This `samesame-rolling` build supersedes the previous `samesame-patience` repo.
Concrete changes that translate to wall-clock wins:

- **Algorithmic.** Patience diff over every file pair was the dominant cost on
  any non-trivial input. Replacing it with a rolling hash + hash-table grouping
  removes the per-pair `O(n·m)` term entirely; detection is now linear in total
  lines and roughly constant-time per window.
- **Hashing.** `xxh3` replaced `blake3` for line hashing (commit `3a10c32`).
  xxh3 is a non-cryptographic hash designed for throughput; for our use
  (comparing line identity, not authenticating bytes) it's the right pick and
  measurably faster on small inputs.
- **Hash maps.** `rustc-hash`'s `FxHashMap` / `FxHashSet` replaced the std
  `SipHash`-backed maps in the hot grouping paths (commit `99892a6`). For the
  many short-lived `u64`-keyed lookups in the rolling-hash pipeline this is a
  noticeable win.
- **Parallel I/O.** File reading and per-line hashing run on a `rayon` pool;
  the rolling-hash detection runs single-threaded after that.
- **Release profile.** `lto = true`, `codegen-units = 1`, `strip = true`,
  `panic = "abort"` — smaller binary, fewer indirect calls, slightly tighter
  inner loops.

In practice this scales further than the original target: a 2.25M-line
TypeScript repo of 11,767 files completes in ~1.2s wall-clock — see the
**Sample performance** section below. Build with `cargo build --release` to
get these gains; debug builds are not representative.

## Sample output

Default text output, with `--match=8`:

```
$ samesame -g "src/**/*.rs" -m 8
Found 9 files to analyze
Loaded 9 text files
Found 3 duplicate groups
=== Duplicate Code Found ===

9 lines duplicated across 2 files:
  src/rolling_hash.rs  lines 197-205
  src/rolling_hash.rs  lines 469-477

---

9 lines duplicated across 2 files:
  src/rolling_hash.rs  lines 287-295
  src/rolling_hash.rs  lines 302-310

---

8 lines duplicated across 2 files:
  src/file.rs  lines 48-55
  src/file.rs  lines 84-91

---

Summary: 9 files analyzed, 3 duplicate groups (26 lines)
```

Pass `-v` to inline the matching source:

```
9 lines duplicated across 2 files:
  src/rolling_hash.rs  lines 197-205
  src/rolling_hash.rs  lines 469-477

   197 |     result.sort_by(|a, b| {
   198 |         b.line_count
   199 |             .cmp(&a.line_count)
   200 |             .then_with(|| a.locations.cmp(&b.locations))
   201 |     });
   202 |
   203 |     result
   204 | }
   205 |
```

Or `-f json` for machine-readable output:

```json
{
  "version": "1.2.1",
  "summary": {
    "files_analyzed": 9,
    "duplicate_groups": 3,
    "total_duplicate_lines": 26
  },
  "duplicates": [
    {
      "lines": 9,
      "locations": [
        { "file": "src/rolling_hash.rs", "start": 197, "end": 205 },
        { "file": "src/rolling_hash.rs", "start": 469, "end": 477 }
      ]
    }
  ]
}
```

Exit codes: `0` if no duplicates, `1` if any are reported, `2` on error — handy
for wiring into CI.

## Sample performance

Test in the `openclaw` repo (a TypeScript monorepo, 11K files with more than
2 million LOC) takes 1.2 seconds:

```
~/workspace/openclaw/openclaw@main% wc -l **/*.ts
...
 2256745 total
~/workspace/openclaw/openclaw@main% ls **/*.ts | wc -l
   11766
~/workspace/openclaw/openclaw@main% time samesame -m 25 -g '**/*.ts'
...
Summary: 11767 files analyzed, 575 duplicate groups (38035 lines)

samesame -m 25 -g '**/*.ts'  0.51s user 6.78s system 608% cpu 1.196 total
```

## Why this helps when refactoring

Duplicated code is one of the cheapest kinds of tech debt to spot and one of
the highest-leverage to fix. samesame helps because:

- **It points at concrete spans, not vague "similar" regions.** Every group
  comes with file paths and line ranges you can open directly. With `-v` you
  see the exact lines, so you can decide in seconds whether a match is a real
  duplicate or coincidental boilerplate (imports, derives, license headers).
- **It scales the threshold to your taste.** `-m 5` surfaces small repeated
  patterns worth a helper function; `-m 30` only flags substantial copy-paste
  worth extracting into a module. Same tool, very different conversations.
- **Multi-file groups make extraction obvious.** When the same 40 lines show
  up in three files, the union-find consolidation reports them as one group —
  exactly the signal you want before pulling them into a shared function.
- **Regex filtering targets one suspicion at a time.** `-r '^fn handle_'`
  restricts results to groups whose first line matches, which is how you ask
  "show me only the duplicated handler bodies" without wading through everything
  else.
- **JSON output plugs into pipelines.** Run it in CI and fail the build when
  `summary.duplicate_groups` exceeds a budget; or feed the locations into an
  editor quickfix list.
- **Fast enough to run on every commit.** Sub-second on typical repos means
  you can put it in a pre-commit hook or a watch loop, so duplicates show up
  while the code is still fresh in your head — which is when extracting them
  is cheapest.

A note on what it doesn't do: samesame finds *identical* runs of lines (modulo
leading/trailing whitespace). It won't detect renamed-variable or restructured
duplicates the way a token-aware tool like a structural clone detector would.
That's a deliberate trade — exact matching is fast, deterministic, and produces
results you can trust without a tuning dial.

## CLI reference

| Flag | Default | Purpose |
| --- | --- | --- |
| `-m, --match <N>` | `5` | Minimum matching lines to report |
| `-d, --dir <PATH>` | — | Directory to scan |
| `-g, --glob <PATTERN>` | `**/*.rs` | Glob pattern under `--dir` |
| `-f, --format <text\|json>` | `text` | Output format |
| `-v, --verbose` | off | Inline the matching source lines |
| `-q, --quiet` | off | Suppress progress / warnings on stderr |
| `-r, --regex <PATTERN>` | — | Keep only groups whose first line matches |

Positional file arguments are also accepted; they're combined with any
directory/glob discovery.
