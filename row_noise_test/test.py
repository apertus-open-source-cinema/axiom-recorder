#!/usr/bin/env python3

import rawpy
import numpy as np
import matplotlib
import matplotlib.colors
import matplotlib.pyplot as plt
matplotlib.use('TkAgg')

img = rawpy.imread("000017.dng")
recorder = rawpy.imread("row_noise_3.dng").raw_image
recorder_green = rawpy.imread("row_noise_5.dng").raw_image
img = img.raw_image.astype(np.uint16)
darkframe = np.fromfile("darkframe_21_03_2024_1x_13.9_2048", dtype=np.float32).reshape(img.shape)

c = np.round(img.astype(np.float32) + 128.0 - darkframe).astype(np.uint16).astype(np.float32) - 128.0

np.set_printoptions(suppress=True)
darkcols = np.hstack([c[:,:8], c[:,-8:]])
row_average = np.mean(darkcols * 0.6, axis=1)

cut = [1400, 1800, 400, 800]
#cut = [1200, 1600, 1200, 1400]

corr = (c.T - row_average.T).T + 128
# this assumes a bayer pattern with 0,0 having a green pixel
green_diffs = []
for lag in range(1, 3):
    diff = np.median(c[::2,::2] - np.roll(c, -lag, axis=0)[::2,lag % 2::2], axis=1)
    diff2 = np.median(c[1::2,1::2] - np.roll(c, -lag, axis=0)[1::2,(lag + 1) % 2::2], axis=1)
    the_diff = np.empty(diff.size + diff2.size)
    the_diff[::2] = diff
    the_diff[1::2] = diff2
    green_diffs.append(the_diff)
    # print(the_diff[:25])

    # np.median(corr[model_parity::2,::2] - np.roll(corr, lag, axis=0)[::2,abs(lag) % 2::2],axis=1,)

fig, (ax1, ax2, ax3, ax4) = plt.subplots(1,4)
norm = matplotlib.colors.LogNorm(100, 300)
ax1.set_title("uncorr")
ax1.imshow((c + 128)[cut[0]:cut[1]:2,cut[2]:cut[3]:2], norm=norm)
ax2.set_title("recorder")
ax2.imshow(recorder[cut[0]:cut[1]:2,cut[2]:cut[3]:2], norm=norm)
ax3.set_title("recorder green")
ax3.imshow(recorder_green[cut[0]:cut[1]:2,cut[2]:cut[3]:2], norm=norm)
ax4.set_title("corr")
ax4.imshow(corr[cut[0]:cut[1]:2,cut[2]:cut[3]:2], norm=norm)
plt.tight_layout()
plt.show()

# print(darkcols[2])

# [9604.051      9604.426         1.1625        2.2875001     1.3125
#     1.7625        5.4750004     5.5875006     0.82500005    0.225
#    -0.63750005   -0.86249995    0.37500006    1.4250001    -0.375
#    -0.37500003    3.825         5.4000006     1.2750001     2.4375
#     2.3625002     3.7125        1.2           1.8000001     0.375     ]


# row: 0000, offset: -18.45
# row: 0001, offset: -18.08
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 1.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = 5.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 6.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = -2.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = -3.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = -1.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 5.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = -1.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 0.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = 14.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = -3.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = 3.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 1.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = 2.0
# [src/nodes_cpu/row_noise_removal.rs:226:33] weights_even[col] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:227:33] src[(even_row + 2 * lag) * width + col] as f32 - BLACK_LEVEL = 6.0
# [src/nodes_cpu/row_noise_removal.rs:228:33] weights_even[col + NUM_DARKCOLS] = 0.0375
# [src/nodes_cpu/row_noise_removal.rs:229:33] src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 -
#     BLACK_LEVEL = -2.0




# row: 0000, lag: 0001, green_diff: -006
# row: 0000, lag: 0002, green_diff: 0002
# row: 0001, lag: 0001, green_diff: 0006
# row: 0001, lag: 0002, green_diff: 0001
# row: 0002, lag: 0001, green_diff: -005
# row: 0002, lag: 0002, green_diff: 0002
# row: 0003, lag: 0001, green_diff: 0006
# row: 0003, lag: 0002, green_diff: 0002
# row: 0004, lag: 0001, green_diff: -005
# row: 0004, lag: 0002, green_diff: -007
# row: 0005, lag: 0001, green_diff: -001
# row: 0005, lag: 0002, green_diff: -007
# row: 0006, lag: 0001, green_diff: -006
# row: 0006, lag: 0002, green_diff: 0010
# row: 0007, lag: 0001, green_diff: 0015
# row: 0007, lag: 0002, green_diff: 0010
# row: 0008, lag: 0001, green_diff: -006
# row: 0008, lag: 0002, green_diff: 0003
# row: 0009, lag: 0001, green_diff: 0009
# row: 0009, lag: 0002, green_diff: 0003
# row: 0010, lag: 0001, green_diff: -006
# row: 0010, lag: 0002, green_diff: -004
# row: 0011, lag: 0001, green_diff: 0003
# row: 0011, lag: 0002, green_diff: -003
# row: 0012, lag: 0001, green_diff: -006
# row: 0012, lag: 0002, green_diff: 0003
# row: 0013, lag: 0001, green_diff: 0009
# row: 0013, lag: 0002, green_diff: 0006
# row: 0014, lag: 0001, green_diff: -002
# row: 0014, lag: 0002, green_diff: -006
# row: 0015, lag: 0001, green_diff: -004
# row: 0015, lag: 0002, green_diff: -010
# row: 0016, lag: 0001, green_diff: -007
# row: 0016, lag: 0002, green_diff: 0007
# row: 0017, lag: 0001, green_diff: 0014
# row: 0017, lag: 0002, green_diff: 0006
# row: 0018, lag: 0001, green_diff: -008
# row: 0018, lag: 0002, green_diff: -003
# row: 0019, lag: 0001, green_diff: 0005
# row: 0019, lag: 0002, green_diff: 0001
# row: 0020, lag: 0001, green_diff: -004
# row: 0020, lag: 0002, green_diff: 0006
# row: 0021, lag: 0001, green_diff: 0009
# row: 0021, lag: 0002, green_diff: 0004
# row: 0022, lag: 0001, green_diff: -005
# row: 0022, lag: 0002, green_diff: 0000
# row: 0023, lag: 0001, green_diff: 0004
# row: 0023, lag: 0002, green_diff: -003
# row: 0024, lag: 0001, green_diff: -007
# row: 0024, lag: 0002, green_diff: 0001
# row: 0025, lag: 0001, green_diff: 0006
# row: 0025, lag: 0002, green_diff: -001
