# std.datetime

Calendar dates and times in UTC, as Unix seconds and as ISO 8601 text. Named
time zones need a zone database, which this module does not carry; an offset
such as `+02:00` in parsed text is applied.

```kestrel
import std.datetime

let t := datetime.fromUnix(0)
print(datetime.format(t))    // 1970-01-01T00:00:00Z
print(t.year)
print(t.month)
print(t.day)
print(datetime.toUnix(t))    // 0
```

## The DateTime record

| Field | Type | Meaning |
|---|---|---|
| `year` | `Int` | Calendar year. |
| `month` | `Int` | 1 to 12. |
| `day` | `Int` | 1 to 31. |
| `hour` | `Int` | 0 to 23. |
| `minute` | `Int` | 0 to 59. |
| `second` | `Int` | 0 to 59. |

A record literal such as `DateTime(2026, 11, 15, 12, 30, 0)` builds one.

## Converting

### `fromUnix(seconds : Int) : DateTime`

The UTC date and time at a Unix timestamp. Negative values work.

| Argument | Meaning |
|---|---|
| `seconds : Int` | Seconds since 1970-01-01T00:00:00Z. |

### `toUnix(t : DateTime) : Int`

The timestamp of a date and time, read as UTC.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The value to convert. |

### `now() : DateTime`

The current UTC time. It carries `Effects` because it reads the clock.

## Arithmetic

### `addSeconds(t : DateTime, seconds : Int) : DateTime`

A new value, `seconds` later.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The starting point. |
| `seconds : Int` | How many seconds to add; negative goes back. |

### `addDays(t : DateTime, days : Int) : DateTime`

A new value, whole days later. Months and leap years are handled.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The starting point. |
| `days : Int` | How many days to add. |

## Reading and writing text

### `format(t : DateTime) : String`

ISO 8601 with the time: `2026-11-15T12:30:00Z`.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The value to write. |

### `formatDate(t : DateTime) : String`

ISO 8601 with the date only: `2026-11-15`.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The value to write. |

### `parse(text : String) : DateTime or Error`

Reads ISO 8601 text. A trailing `Z` or an offset is applied and the result is
UTC.

| Argument | Meaning |
|---|---|
| `text : String` | The text to read. |

```kestrel
let t := datetime.parse("2026-11-15T12:30:00+02:00")?
print(t.hour)   // 10
```

## Calendar facts

### `isLeapYear(year : Int) : Bool`

True for a leap year under the Gregorian rule.

| Argument | Meaning |
|---|---|
| `year : Int` | The year. |

### `daysInMonth(year : Int, month : Int) : Int`

28, 29, 30 or 31.

| Argument | Meaning |
|---|---|
| `year : Int` | The year, for February. |
| `month : Int` | The month, 1 to 12. |

### `dayOfWeek(t : DateTime) : Int`

The weekday, with Monday as `0`.

| Argument | Meaning |
|---|---|
| `t : DateTime` | The date. |

## Notes

- Everything is UTC. Convert with an offset before or after, not in between.
- There is no time-zone database, so `Europe/Berlin` is not understood; the
  numeric offset is.
