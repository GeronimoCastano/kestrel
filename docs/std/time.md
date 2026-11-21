# std.time

Wall time, a steady clock, and sleeping.

```kestrel
import std.time

let started := time.monotonic()
time.sleep(0.25)
let elapsed := time.monotonic() - started
print(f"{elapsed / 1_000_000.0:.3f} s")
```

## Functions

### `now() : double`

The current wall-clock time in seconds since the Unix epoch. It carries
`Effects`, and it can jump: the system clock is adjusted by time servers and by
the user. Use it to stamp an event, not to measure a duration.

```kestrel
let stamp := time.now()
print(f"{stamp:.0f}")
```

### `monotonic() : Int`

Nanoseconds from a clock that never goes backwards. The origin is unspecified,
so only differences between two calls are meaningful.

```kestrel
let start := time.monotonic()
doWork()
let nanoseconds := time.monotonic() - start
```

### `sleep(seconds : double) : void`

Suspends the program for at least `seconds`. The argument is fractional, so
`time.sleep(0.05)` waits fifty milliseconds.

| Argument | Meaning |
|---|---|
| `seconds : double` | How long to wait. Negative values return at once. |

```kestrel
time.sleep(0.5)
```

## Notes

- `now` is for dates and timestamps; `monotonic` is for measuring elapsed time.
- Sleeping blocks the whole program, tasks included. A concurrent program that
  sleeps by accident looks slow, not busy.
