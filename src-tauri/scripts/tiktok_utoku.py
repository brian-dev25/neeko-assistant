# SPDX-License-Identifier: GPL-3.0-only
# Python adaptation of ut0ku/120fps-method's patch_mvhd / patch_mdhd.
# Original reference and license: ../vendor/tiktok-engines/utoku/.
def patch(data, divider):
    count = 0
    for name in (b'mvhd', b'mdhd'):
        start = 0
        while (index := data.find(name, start)) >= 0:
            pos = index - 4
            if pos < 0 or pos + 24 >= len(data):
                break
            version = data[pos + 8]
            # Preserve upstream offsets, including its version-1 behavior.
            timescale = pos + (28 if name == b'mdhd' and version != 0 else 20)
            duration = timescale + (8 if name == b'mdhd' and version != 0 else 4)
            width = 4 if version == 0 else 8
            if duration + width > len(data):
                raise ValueError('Cabecera MP4 truncada para ut0ku.')
            old = int.from_bytes(data[timescale:timescale + 4], 'big')
            data[timescale:timescale + 4] = max(1, old // divider).to_bytes(4, 'big')
            old = int.from_bytes(data[duration:duration + width], 'big')
            data[duration:duration + width] = (old // divider).to_bytes(width, 'big')
            count += 1
            start = index + 4
    if not count:
        raise ValueError('No se encontraron boxes mvhd/mdhd para ut0ku.')
    return count
