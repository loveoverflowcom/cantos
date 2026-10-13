# Synthetic image fixtures

No real artwork or downloaded image is included. All fixtures are 2x3 pixels with alternating
red/green RGB pixels (or solid red RGBA16) and were generated locally on 2026-10-13.

- `python-rgb-2x3.png`: Python `struct`/`zlib`, 8-bit RGB; 74 encoded bytes;
  SHA-256 `7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276`.
- `python-rgba16-2x3.png`: Python `struct`/`zlib`, 16-bit RGBA; 77 encoded bytes;
  SHA-256 `1e2725064b73e8bdc0c7056ae8bf2d7f17dbcae5a7dac2d5f13fc398076c357d`.
- `pillow-rgb-2x3.jpg`: Pillow 12.3.0, RGB JPEG, quality90, baseline, no optimization;
  667 encoded bytes; SHA-256 `a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46`.

Hashes were computed independently with Python `hashlib`. The PNG construction writes the PNG
signature, IHDR with `(width=2,height=3,depth=8,color=2)` or `(depth=16,color=6)`, one zlib IDAT
with unfiltered rows, and IEND. Every chunk has a Python `zlib.crc32` checksum. The JPEG was
generated with `Image.new('RGB',(2,3))`, `putdata([(255,0,0),(0,255,0)]*3)` and
`save(format='JPEG',quality=90,progressive=False,optimize=False)`.

Hostile envelopes, large output, compressed ICC overload, EXIF rotation and animation markers
are synthesized in the ordinary Rust test suite with separate test-only framing/CRC/zlib code.
Actual acceptance always uses the pinned production image decoder, never these encoders.
