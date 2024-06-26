#!/usr/bin/env python3

import sys
import numpy as np
from scipy.optimize import curve_fit
from dataclasses import dataclass
import numba
import argparse


def eprint(*args):
    print(*args, file=sys.stderr)


np.set_printoptions(suppress=True)

NUM_DARKCOLS = 8
BLACK_LEVEL = 128


@dataclass
class ModelHalfWeights:
    # [f32; 2 * NUM_DARKCOLS]
    dark_col_mean_weights: np.ndarray
    # Vec<[f32; 2]>, first pair for lag 1, then pair for lag 2, etc
    green_diff_weights: np.ndarray
    # Vec<[[f32; 2 * NUM_DARKCOLS]; 2]>, first for lag 0, then pair for lag 1, etc
    dark_col_row_weights: np.ndarray
    offset: float

    def pack_weights(self) -> np.ndarray:
        return np.concatenate(
            [
                np.ravel(self.green_diff_weights),
                np.ravel(self.dark_col_row_weights),
                np.ravel(self.dark_col_mean_weights),
                [self.offset],
            ]
        )


@dataclass
class ModelWeights:
    weights_even: ModelHalfWeights
    weights_odd: ModelHalfWeights

    def serialize(self) -> str:
        import yaml

        def ndarray_representer(dumper: yaml.Dumper, array: np.ndarray) -> yaml.Node:
            if array.size > 0:
                return dumper.represent_list(array.tolist())
            else:
                return dumper.represent_scalar("tag:yaml.org,2002:null", "")

        yaml.add_representer(np.ndarray, ndarray_representer)
        return yaml.dump(self)


def fast_median(array, axis):
    kth = array.shape[axis] // 2
    array.partition(kth, axis=axis)
    index = [slice(None)] * array.ndim
    index[axis] = kth
    return array[tuple(index)].copy()


@dataclass
class ModelParameters:
    num_green_lags: int
    num_dark_col_rows: int
    has_dark_column: int

    def nparams(self) -> int:
        # +1 for offset
        return (
            self._nparams_dark_col_mean()
            + self._nparams_dark_col_rows()
            + self._nparams_green_diffs()
            + 1
        )

    def _nparams_dark_col_rows(self) -> int:
        if self.num_dark_col_rows == 0:
            return 0
        return (self.num_dark_col_rows * 2 - 1) * 2 * NUM_DARKCOLS * 2

    def _nparams_dark_col_mean(self) -> int:
        if self.has_dark_column:
            return 2 * NUM_DARKCOLS
        else:
            return 0

    def _nparams_green_diffs(self) -> int:
        return self.num_green_lags * 2

    def initial_weights(self) -> list[float]:
        # offset initialized with 0 works better
        return [0.0] * (self.nparams() - 1) + [0.0]

    def unpack_weights(self, weights: np.ndarray) -> ModelHalfWeights:
        pos = 0

        nparam = self._nparams_green_diffs()
        green_diff_weights = weights[pos : pos + nparam].reshape((-1, 2))
        pos += nparam

        nparam = self._nparams_dark_col_rows()
        dark_col_row_weights = weights[pos : pos + nparam].reshape(
            (-1, 2, 2 * NUM_DARKCOLS)
        )
        pos += nparam

        nparam = self._nparams_dark_col_mean()

        dark_col_mean_weights = weights[pos : pos + nparam]
        pos += nparam
        offset = float(weights[pos])

        return ModelHalfWeights(
            green_diff_weights=green_diff_weights,
            dark_col_row_weights=dark_col_row_weights,
            dark_col_mean_weights=dark_col_mean_weights,
            offset=offset,
        )

    # signature to make it work with scipy curve_fit
    # given the hyperparameters, weights and input data x packed by pack_data, produce the row averages
    def compute_fit(self, x, *weights) -> np.ndarray:
        return x @ weights[:-1] + weights[-1]

    # TODO(robin): maybe support for different bayer pattern than green at pixel 0,0 ?
    def build_data(
        self, darkframes: np.ndarray, darkframe_mean=None
    ) -> tuple[np.ndarray, np.ndarray]:
        eprint("calculating model inputs")
        n_darkframes = darkframes.shape[0]
        if darkframe_mean is None:
            darkframe_mean = np.mean(darkframes, axis=0)
        frame_height = darkframe_mean.shape[0]
        frame_width = darkframe_mean.shape[1]

        eprint("subtracting mean")

        @numba.njit(parallel=True)
        def _sub_mean_darkframe(darkframes):
            for frame in numba.prange(n_darkframes):
                darkframes[frame] += BLACK_LEVEL
                darkframes[frame] = np.round(
                    darkframes[frame] - darkframe_mean, 0, darkframes[frame]
                )

        _sub_mean_darkframe(darkframes)

        darkframes.shape = (-1, frame_width)

        flat_darkframes = darkframes
        del darkframes

        eprint("calculating row means")
        row_means = (
            np.mean(flat_darkframes[:, NUM_DARKCOLS:-NUM_DARKCOLS], axis=1)
            - BLACK_LEVEL
        )

        eprint("getting dark cols")
        dark_col_rows = (
            np.repeat(
                np.hstack(
                    [
                        flat_darkframes[:, :NUM_DARKCOLS],
                        flat_darkframes[:, -NUM_DARKCOLS:],
                    ]
                ).reshape((-1, 2 * NUM_DARKCOLS * 2)),
                2,
                axis=0,
            )
            - BLACK_LEVEL
        )

        eprint("calculating green diffs")
        # here we cheat a bit. roll would be more nice, however it is slow
        # non numba version:
        # for lag in range(1, max_lag + 1):
        #     diffs = []
        #     for frame in tqdm(range(n_darkframes)):
        #         df = flat_darkframes[frame * frame_height: (frame + 1) * frame_height]

        #         diff_even = fast_median(df[0:-max_lag:2,0::2] - df[0 + lag:(-max_lag + lag) or None:2,(lag + 0) % 2::2], axis=1)
        #         diff_odd = fast_median(df[1:-max_lag:2,1::2] - df[1 + lag:(-max_lag + lag) or None:2,(lag + 1) % 2::2], axis=1)

        #         diff = np.zeros(diff_even.size + diff_odd.size + max_lag)
        #         diff[:-max_lag:2] = diff_even
        #         diff[1:-max_lag:2] = diff_odd
        #         diffs.append(diff)

        #     diff = np.concatenate(diffs)
        #     print(diff)

        #     green_diffs.append(diff)

        max_lag = self.num_green_lags

        @numba.njit(parallel=True)
        def _calculate_green_diffs():
            green_diffs = []
            for lag in range(1, max_lag + 1):
                diff = np.zeros(frame_height * n_darkframes, dtype=np.int16)
                for frame in numba.prange(n_darkframes):
                    df = flat_darkframes[
                        frame * frame_height : (frame + 1) * frame_height
                    ]

                    for row in range(frame_height - max_lag):
                        diff[frame_height * frame + row] = np.median(
                            df[row, (row % 2) :: 2]
                            - df[row + lag, (lag + row) % 2 :: 2]
                        )
                green_diffs.append(diff)
            return green_diffs

        green_diffs = _calculate_green_diffs()

        if len(green_diffs) > 0:
            green_diffs = np.stack(green_diffs)

        del flat_darkframes
        dark_col_means = np.mean(
            dark_col_rows[::2].reshape((-1, frame_height, 2 * NUM_DARKCOLS)), axis=1
        )
        dark_col_means = np.repeat(dark_col_means, frame_height, axis=0)
        packed_data = self.pack_data(green_diffs, dark_col_rows, dark_col_means)

        # some of the rows have invalid data, those are uncorrectable
        num_uncorrectable = self._num_uncorrectable()

        if num_uncorrectable > 0:
            single_frame_mask = np.ones(frame_height, dtype=np.bool8)
            single_frame_mask[:num_uncorrectable] = 0
            single_frame_mask[-num_uncorrectable:] = 0
            mask = np.tile(single_frame_mask, n_darkframes)

            data = packed_data[mask]
            row_means = row_means[mask]
        else:
            data = packed_data

        return (data, row_means)

    # this return data for all rows, even the uncorrectable ones, make sure to mask those
    def pack_data(
        self,
        green_diffs: np.ndarray,
        dark_col_rows: np.ndarray,
        dark_col_means: np.ndarray,
    ) -> np.ndarray:
        data = []

        # first green lags:
        # first we have the weight for lag -1, then lag 1 then lag -2, then lag 2, etc
        for lag in range(0, self.num_green_lags):
            # green_diffs[lag] is row minus row + lag
            # get median of row - row minus lag by taking the negative median of row minus lag - lag
            # lag zero means we want to shift by one
            data.append(-np.roll(green_diffs[lag], lag + 1, axis=0).reshape(-1, 1))
            data.append(green_diffs[lag].reshape(-1, 1))

        # again, first lag 0, then lag -1, then lag 1, then lag -2, then lag 2, etc
        for lag in range(0, self.num_dark_col_rows):
            if lag == 0:
                data.append(dark_col_rows)
            else:
                # we interpret the lags for the dark cols as blocks of two rows,
                # as we use both the respective even and the odd row for each row
                data.append(np.roll(dark_col_rows, 2 * lag, axis=0))
                data.append(np.roll(dark_col_rows, -2 * lag, axis=0))

        if self.has_dark_column:
            data.append(dark_col_means)

        return np.hstack(data)

    def fit_model(
        self, darkframes, darkframe_mean=None, use_odd_even=True
    ) -> ModelWeights:
        x, row_means = self.build_data(darkframes, darkframe_mean)
        del darkframes

        def fit_single_model(x, row_means) -> ModelWeights:
            p0 = self.initial_weights()
            weights, _ = curve_fit(
                lambda x, *weights: self.compute_fit(x, *weights),
                x,
                row_means,
                p0=p0,
                method="lm",
            ) 
            self.evaluate_model(row_means, weights, x)
            return weights

        if use_odd_even == False:
            eprint("creating combined even odd model")
            weights = fit_single_model(x, row_means)
            weights_even = weights_odd = weights
        else:
            # we cut of the uncorrectable rows from the top and the bottom
            # ensure, that we have the correct even odd parity when going from x and row_mean to weights_{even, odd}
            parity = self._num_uncorrectable() % 2
            eprint("creating even model:")
            weights_even = fit_single_model(x[parity::2], row_means[parity::2])
            eprint("creating odd model:")
            weights_odd = fit_single_model(
                x[(parity + 1) % 2 :: 2], row_means[(parity + 1) % 2 :: 2]
            )

        return ModelWeights(
            weights_even=self.unpack_weights(weights_even),
            weights_odd=self.unpack_weights(weights_odd),
        )

    def evaluate_model(self, row_means, weights, x):
        eprint("evaluating model")
        initial_residual = (np.sum(row_means**2) / row_means.shape[0]) ** 0.5
        fitted_row_means = self.compute_fit(x, *weights)
        fit_residual = (
            np.sum((row_means - fitted_row_means) ** 2) / row_means.shape[0]
        ) ** 0.5
        eprint(
            f"average quadratic row deviation before correction: {initial_residual}, after: {fit_residual}"
        )

    def _num_uncorrectable(self):
        return max(self.num_green_lags, (self.num_dark_col_rows - 1) * 2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="generate a row noise removal model for the axiom recorder using a given "
    )
    parser.add_argument(
        "darkframe_stack",
        help="the darkframe stack as a folder of dng files",
    )
    parser.add_argument(
        "-c", "--count", help="the number of darkframes to use for training the model"
    )

    parser.add_argument(
        "--green_diff_lags",
        help="the number of green lags to consider in the model."
        "Set to 0 to disable green diffs",
        default=0,
        type=int,
    )
    parser.add_argument(
        "--dark_column_rows",
        help="the number of dark column rows to use (1=2, 2=6, 3=10, 4=14, ...). Set to 0 to disable dark columns",
        default=0,
        type=int,
    )
    parser.add_argument(
        "--combined_model",
        help="use the same model for odd and even rows",
        default=False,
        action="store_true"
    )

    args = parser.parse_args()

    from pathlib import Path
    import rawpy

    eprint("reading darkframe stack")
    dngs = list(Path(args.darkframe_stack).glob("*.dng"))
    with rawpy.imread(str(dngs[0])) as raw:
        h, w = raw.raw_image.shape
    darkframes = np.empty((len(dngs), h, w), dtype=np.int16)
    for i, dng in enumerate(dngs):
        with rawpy.imread(str(dng)) as raw:
            darkframes[i] = raw.raw_image

    eprint("calculating darkframe mean")
    mean = np.mean(darkframes, axis=0)

    if args.green_diff_lags == 0 and args.dark_column_rows == 0:
        eprint("both green lags and dark column rows disabled, cannot fit a model with zero parameters")
        exit(1)

    # TODO(robin): check dark column values, they are broken
    # TODO(robin): why with only dark cols is there a asymmetry between even and odd?
    model = ModelParameters(
        num_green_lags=args.green_diff_lags, num_dark_col_rows=args.dark_column_rows, has_dark_column=False
    )
    weights = model.fit_model(darkframes=darkframes, darkframe_mean=mean, use_odd_even=not args.combined_model)
    print(weights.serialize())
