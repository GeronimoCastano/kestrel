# std.encoding

Base64, hex, CRC-32 and SHA-256, all over the UTF-8 bytes of a `String`.

```kestrel
import std.encoding

print(encoding.base64Encode("hello"))    // aGVsbG8=
print(encoding.hexEncode("hello"))       // 68656c6c6f
print(encoding.crc32("hello"))           // 907060870
print(encoding.sha256("hello"))
```

```text
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

## Functions

### `base64Encode(text : String) : String`

Standard Base64 with `+` and `/`, padded with `=`.

| Argument | Meaning |
|---|---|
| `text : String` | The text to encode, as UTF-8 bytes. |

### `base64Decode(text : String) : String or Error`

Decodes Base64 back to text. Input that is not valid Base64, or that decodes to
bytes which are not UTF-8, is an error.

| Argument | Meaning |
|---|---|
| `text : String` | The Base64 text. |

### `hexEncode(text : String) : String`

Lower-case hexadecimal, two digits per byte.

| Argument | Meaning |
|---|---|
| `text : String` | The text to encode. |

### `hexDecode(text : String) : String or Error`

Decodes hexadecimal text. An odd number of digits or a non-hex character is an
error.

| Argument | Meaning |
|---|---|
| `text : String` | The hexadecimal text. |

### `crc32(text : String) : Int`

The CRC-32 checksum, as an unsigned 32-bit value in an `Int`.

| Argument | Meaning |
|---|---|
| `text : String` | The text to check. |

### `sha256(text : String) : String`

The SHA-256 digest, as 64 lower-case hexadecimal characters.

| Argument | Meaning |
|---|---|
| `text : String` | The text to hash. |

## Notes

- Every function works on the UTF-8 bytes of the string, so a multi-byte
  character contributes several bytes.
- `hexDecode(hexEncode(x))` and `base64Decode(base64Encode(x))` return `x`.
- SHA-256 is in the compiler's own SHA-256 implementation; there is no OpenSSL
  dependency.
