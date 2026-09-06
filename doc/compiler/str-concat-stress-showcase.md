# Showcase: concat leak before vs after (issue #46)

**Yes — the stress case is bounded after the fix.**

This page is for screenshots. Program: [`doc/examples/loop/str-concat-stress.hyp`](../examples/loop/str-concat-stress.hyp).

```hyper
let mut s = ""
for i in range(80000):
    s = s + "x"
print(s.len())
```

Correct result is `80000`. The leak is **RSS**, not the printed length.

## How RSS was measured

While `hyper compile` ran, `/proc/<pid>/status` `VmRSS` was sampled every ~10 ms (Linux). Peak is the maximum sample. Same machine, debug JIT binary.

```text
VmRSS:    <kilobytes> kB
```

Unfixed compile path keeps every prefix (`""`, `"x"`, `"xx"`, …): about \(n(n+1)/2\) bytes ≈ **3.2 GB** of string data at n = 80 000, plus malloc overhead. That matches the before peak.

## BEFORE (unfixed JIT) — unbounded

Captured 2026-09-05 against `target/debug/hyper` **without** consume-on-concat. Samples kept climbing until the loop finished.

```text
$ hyper compile doc/examples/loop/str-concat-stress.hyp
80000

exit          0
stdout        80000
rss_samples   [576, 8452, 9916, 24032, 38856, 52568, 70004, 82132, …]
              [3335532, 3337552, 3344660, 3344660, 3348804, 3349572, 3352104]
peak_rss_kb   3352104          # ~3.35 GB and still rising at the last samples
n_samples     1115
runtime       ~13 s
```

**Screenshot this block as BEFORE.** RSS in the millions of kB, last samples still increasing.

## AFTER (fixed JIT) — bounded

Captured 2026-09-06 with consume flags (`s = s + "x"` frees the previous owned `s`). After Cranelift JIT warmup, RSS sits near **10.8 MB** and does not track the 80 000 iterations.

```text
$ hyper compile doc/examples/loop/str-concat-stress.hyp
80000

exit          0
stdout        80000
rss_samples   first: [368, 9332, 10448, 10488, 10496]
              last:  [10764, 10788, 10764, 10764, 10764]
peak_rss_kb   10788            # ~10.8 MB, flat after startup
n_samples     140
runtime       ~3 s
```

**Screenshot this block as AFTER.** Peak is ~300× smaller; last samples are flat (~10 764 kB).

AOT `--emit-exe` of the same program peaked at **~1.9 MB** (no JIT image).

## Take your own AFTER screenshot

From the repo root (Linux):

```bash
hyper compile doc/examples/loop/str-concat-stress.hyp
```

To print live RSS like the tables above:

```bash
python3 - <<'PY'
import subprocess, time, sys
cmd = [sys.argv[1] if len(sys.argv) > 1 else "hyper",
       "compile", "doc/examples/loop/str-concat-stress.hyp"]
p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
peak, samples = 0, []
while p.poll() is None:
    try:
        for line in open(f"/proc/{p.pid}/status"):
            if line.startswith("VmRSS:"):
                kb = int(line.split()[1])
                samples.append(kb)
                peak = max(peak, kb)
                break
    except FileNotFoundError:
        break
    time.sleep(0.01)
out, err = p.communicate()
print("stdout     ", out.decode().strip())
print("exit       ", p.returncode)
print("first rss  ", samples[:6])
print("last rss   ", samples[-6:])
print("peak_rss_kb", peak)
if err:
    print(err.decode())
PY
```

You cannot reproduce BEFORE without an old binary: `s = s + "x"` now consumes the previous heap string. Use the BEFORE transcript on this page for the unfixed screenshot.

## What “bounded” means here

| | BEFORE | AFTER |
|---|--------|--------|
| Peak RSS @ 80k concats | ~3.35 GB, climbing | ~10.8 MB JIT / ~1.9 MB AOT, flat |
| Heap strings kept | all prefixes (quadratic) | current `s` only (linear in the final length, 80 000 bytes) |
| Printed length | 80000 | 80000 |

Acceptance: **stress case no longer unbounded-leaks (clearly bounded).**
