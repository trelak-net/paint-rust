#!/usr/bin/env python3
"""Generate original AVIF interoperability inputs with FFmpeg/libaom."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent

def box(kind, payload):
    return (len(payload) + 8).to_bytes(4, 'big') + kind + payload

def transform_properties(source, additions):
    # This helper intentionally handles only the fixed FFmpeg fixture layout.
    # Its iloc uses version zero, 32-bit absolute offsets and no base offset.
    extra = sum(len(box(kind, value)) + 1 for kind, value in additions)
    def rewrite(data):
        result = b''
        while data:
            size = int.from_bytes(data[:4], 'big')
            assert 8 <= size <= len(data)
            kind, payload = data[4:8], data[8:size]
            if kind == b'meta':
                payload = payload[:4] + rewrite(payload[4:])
            elif kind == b'iprp':
                payload = rewrite(payload)
            elif kind == b'ipco':
                payload += b''.join(box(k, v) for k, v in additions)
            elif kind == b'ipma':
                assert payload == bytes.fromhex('000000000000000100010401028304')
                payload = payload[:10] + bytes([4 + len(additions)]) + payload[11:]
                payload += bytes(0x80 | (5+i) for i in range(len(additions)))
            elif kind == b'iloc':
                assert payload[:14] == bytes.fromhex('0000000044000001000100000001')
                offset = int.from_bytes(payload[14:18], 'big')
                payload = payload[:14] + (offset + extra).to_bytes(4, 'big') + payload[18:]
            result += box(kind, payload)
            data = data[size:]
        return result
    return rewrite(source)

with tempfile.TemporaryDirectory() as temporary:
    source = Path(temporary) / 'input.ppm'
    rgb = bytes(c for y in range(6) for x in range(8)
                for c in ((255, 0, 0) if x < 3 else (0, 255, 0) if y < 3 else (0, 0, 255)))
    source.write_bytes(b'P6\n8 6\n255\n' + rgb)
    base = ['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-i', str(source),
            '-frames:v', '1', '-c:v', 'libaom-av1', '-cpu-used', '8', '-crf', '0']
    subprocess.run(base + ['-pix_fmt', 'gbrp', '-color_primaries', 'bt709', '-color_trc',
                          'iec61966-2-1', '-colorspace', 'rgb', '-color_range', 'pc',
                          str(ROOT / 'rgb-lossless.avif')], check=True)
    subprocess.run(base + ['-pix_fmt', 'yuv444p', '-color_primaries', 'bt2020', '-color_trc',
                          'smpte2084', '-colorspace', 'bt2020nc', '-color_range', 'pc',
                          str(ROOT / 'hdr-declaration.avif')], check=True)
    source.write_bytes(b'P6\n17000 1\n255\n' + bytes(17000 * 3))
    subprocess.run(base + ['-pix_fmt', 'gbrp', '-color_primaries', 'bt709', '-color_trc',
                          'iec61966-2-1', '-colorspace', 'rgb', '-color_range', 'pc',
                          str(ROOT / 'oversized-width.avif')], check=True)

base = (ROOT / 'rgb-lossless.avif').read_bytes()
for name, additions in [
    ('rotated', [(b'irot', b'\x01')]),
    ('mirrored', [(b'imir', b'\x01')]),
    ('rotated-mirrored', [(b'irot', b'\x01'), (b'imir', b'\x00')]),
]:
    (ROOT / (name + '.avif')).write_bytes(transform_properties(base, additions))

# Set a deliberate HDR container declaration without changing the synthetic pixels.
p = ROOT / 'hdr-declaration.avif'
b = bytearray(p.read_bytes())
offset = b.index(b'nclx')
b[offset + 4:offset + 8] = bytes.fromhex('00090010')
p.write_bytes(b)
