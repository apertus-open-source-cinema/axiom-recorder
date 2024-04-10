#!/usr/bin/env python3

from dataclasses import dataclass
import zstandard
import numpy as np
from tqdm import tqdm
from os import SEEK_SET

resolutions = [
    (3840, 2160),
    (4096, 2160)
]

markers = {
    0x55: 0xAA,
    0xAA: 0x55
}

bitdepth = 12
max_frame_bytes = max(width * height * bitdepth // 8 for width, height in resolutions)

def get_size(filename):
    with open(filename, "rb") as f:
        header = f.read(18) # 18 bytes: 4 bytes magic + 2 to 14 bytes frame header
        return zstandard.frame_content_size(header)


@dataclass
class CornerMarker:
    frame_number: int
    wrsel: int
    marker: int

    @staticmethod
    def parse(rgb):
        return CornerMarker(frame_number = rgb[0], wrsel = rgb[1], marker = rgb[2])

def read_darkframes(filename, *, count = None, progress = False, asbytes=False):
    def get_corners(data, width: int, height: int) -> list[CornerMarker]:
        return [
            CornerMarker.parse(data[0:]),
            CornerMarker.parse(data[3 * (width - 1):]),
            CornerMarker.parse(data[3 * width * (height - 1):]),
            CornerMarker.parse(data[3 * (width * height - 1):]),
        ]

    def check_corners(corners: list[CornerMarker]):
        same_framenumber = len(set(corner.frame_number for corner in corners)) == 1
        markers = set(corner.marker for corner in corners)
        same_markers = len(markers) == 1
        return same_framenumber and same_markers, markers.pop()

    with open(filename, 'rb') as fh:
        dctx = zstandard.ZstdDecompressor()
        with dctx.stream_reader(fh) as reader:
            test_data = reader.read(2 * max_frame_bytes)
            for width, height in resolutions:
                for offset in [0, width * height * 12 // 8]:
                    lb = bitdepth * width // 8
                    td = np.frombuffer(test_data[offset:], dtype=np.uint8).reshape((-1, lb))
                    corners_top = get_corners(np.ravel(td[::2, :]), lb // 3, height // 2)
                    corners_bottom = get_corners(np.ravel(td[1::2, :]), lb // 3, height // 2)
                    # print(corners_top)
                    # print(corners_bottom)

                    top_valid, top_marker = check_corners(corners_top)
                    bottom_valid, bottom_marker = check_corners(corners_bottom)

                    valid = top_valid and bottom_valid and top_marker in markers and markers[top_marker] == bottom_marker
                    if valid:
                        break
                else:
                    continue
                break
            else:
                # raise Exception("could not find valid corner markers for any resolution")
                width = 4096
                height = 2160

    # print(f"using width = {width} and height = {height}")
    with open(filename, 'rb') as fh:
        dctx = zstandard.ZstdDecompressor()
        with dctx.stream_reader(fh) as reader:
            if count is None:
                count = 8 * get_size(filename) // height // width // bitdepth
            if not asbytes:
                darkframes = np.zeros((count, width * height), dtype=np.int16)
            else:
                darkframes = []

            bytes_per_frame = width * height * bitdepth // 8
            lb = bitdepth * width // 8
            for frame in range(count):
                b = reader.read(bytes_per_frame)
                if len(b) != bytes_per_frame:
                    break
                a = np.frombuffer(b, dtype=np.uint8).astype(np.int16)

                td = a.reshape((-1, lb))

                corners_top = get_corners(np.ravel(td[::2, :]), lb // 3, height // 2)
                corners_bottom = get_corners(np.ravel(td[1::2, :]), lb // 3, height // 2)
                # print(frame)
                # print(corners_top)
                # print(corners_bottom)

                if asbytes:
                    darkframes.append(td)
                else:
                    darkframes[frame,::2] += a[::3] << 4
                    darkframes[frame,::2] += a[1::3] >> 4
                    darkframes[frame,1::2] += (a[1::3] & 0xf) << 8
                    darkframes[frame,1::2] += a[2::3]

            if not asbytes:
                darkframes = darkframes[:frame,:]


    if not asbytes:
        darkframes = darkframes.reshape((frame, height, width))
    return darkframes
