use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};


use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
};
use async_trait::async_trait;


#[derive(serde::Deserialize)]
pub struct PerHalfWeights {
    dark_col_mean_weights: Option<[f32; 2 * NUM_DARKCOLS]>,
    #[serde(default)]
    green_diff_weights: Vec<[f32; 2]>,

    // index -> lags
    // 0, 1, 2, 3, 4 -> 0, -1, 1, -2, 2
    #[serde(default)]
    dark_col_row_weights: Vec<[[f32; 2 * NUM_DARKCOLS]; 2]>,
    #[serde(default)]
    offset: f32,
}

#[derive(serde::Deserialize)]
pub struct RowNoiseRemovalModel {
    weights_odd: PerHalfWeights,
    weights_even: PerHalfWeights,
}

impl PerHalfWeights {
    fn num_green_lags(&self) -> usize {
        self.green_diff_weights.len()
    }

    fn num_dark_cols(&self) -> usize {
        1 + self.dark_col_row_weights.len() / 2
    }

    fn has_dark_col_means(&self) -> bool { self.dark_col_mean_weights.is_some() }
}

impl RowNoiseRemovalModel {
    fn num_green_lags(&self) -> usize { self.weights_even.num_green_lags() }

    fn num_dark_cols(&self) -> usize { self.weights_even.num_dark_cols() }

    fn has_dark_col_means(&self) -> bool { self.weights_even.has_dark_col_means() }

    fn num_uncorrectable(&self) -> usize {
        return self.num_green_lags().max((self.num_dark_cols() - 1) * 2);
    }
}

pub struct RowNoiseRemoval {
    model: RowNoiseRemovalModel,
    input: InputProcessingNode,
    context: ProcessingContext,
}

impl Parameterizable for RowNoiseRemoval {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("model", Mandatory(StringParameter))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self> {
        let model_path: String = parameters.take("model")?;
        Ok(Self {
            input: parameters.take("input")?,
            context: context.clone(),
            model: serde_yaml::from_str(&std::fs::read_to_string(&model_path).with_context(|| format!("Failed to read model from {}", model_path))?)?,
        })
    }
}


fn get_col_parity_for_row(interp: Raw, row: usize) -> usize {
    let parity_for_even_row = if interp.cfa.red_in_first_col && interp.cfa.red_in_first_row {
        1
    } else if !interp.cfa.red_in_first_col && !interp.cfa.red_in_first_row {
        1
    } else {
        0
    };

    if row % 2 == 0 {
        parity_for_even_row
    } else {
        1 - parity_for_even_row
    }
}


// per side
const NUM_DARKCOLS: usize = 8;
const BLACK_LEVEL: f32 = 128f32;

#[async_trait]
impl ProcessingNode for RowNoiseRemoval {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let frame = self.input.pull(request).await?;
        let frame = self.context.ensure_cpu_buffer::<Raw>(&frame).unwrap();
        let interp = frame.interp;
        let width = interp.width as usize;
        let height = interp.height as usize;
        assert_eq!(frame.interp.bit_depth, 16);

        // dark_col_mean_weights: Option<[f32; 16]>,
        // green_diff_weights: Option<Vec<f32>>,
        // dark_col_row_weights: Option<Vec<f32>>,

        let model = &self.model;

        let mut dark_col_mean_offset_even = 0f32;
        let mut dark_col_mean_offset_odd = 0f32;

        let mut green_diffs =
            vec![vec![0f32; (height - model.num_uncorrectable())]; model.num_green_lags()];

        let slice = frame.storage.as_slice(|frame| {
            let frame: &[u16] = bytemuck::cast_slice(frame);

            if model.has_dark_col_means() {
                let mut dark_col_means = [0f32; 2 * NUM_DARKCOLS];
                for row in 0..height {
                    for col in 0..NUM_DARKCOLS {
                        dark_col_means[col] += frame[row * width + col] as f32 - BLACK_LEVEL;
                    }

                    for col in 0..NUM_DARKCOLS {
                        dark_col_means[col + NUM_DARKCOLS] +=
                            frame[row * width + (width - NUM_DARKCOLS + col)] as f32 - BLACK_LEVEL;
                    }
                }
                for col in 0..16 {
                    dark_col_means[col] /= height as f32;
                }

                if let Some(weights) = model.weights_even.dark_col_mean_weights {
                    dark_col_mean_offset_even =
                        weights.iter().zip(dark_col_means).map(|(a, b)| a * b).sum();
                }
                if let Some(weights) = model.weights_odd.dark_col_mean_weights {
                    dark_col_mean_offset_odd =
                        weights.iter().zip(dark_col_means).map(|(a, b)| a * b).sum();
                }
            }

            for row in 0..(height - model.num_uncorrectable()) {
                for lag in 0..model.num_green_lags() {
                    let lag = lag + 1;
                    let mut diffs = Vec::new();


                    /* Gedankenstütze:
                     *
                     * |  B  |  G  |  B  |  G  |
                     * |  G  |  R  |  G  |  R  |
                     * |  B  |  G  |  B  |  G  |
                     * |  G  |  R  |  G  |  R  |
                     diffs.len() / 2*
                     */


                    // this is a offset, so that `col + col_parity` in the loop below are the
                    // indices of the green values for `row`
                    // let col_parity = get_col_parity_for_row(interp, row);
                    let col_parity = get_col_parity_for_row(interp, row);
                    // this is a offset, so that `col + col_lag_parity` in the loop below are the
                    // indices of the green values for `row + lag`
                    let col_lag_parity = get_col_parity_for_row(interp, row + lag);

                    for col in (0..width).step_by(2) {
                        diffs.push(
                            frame[row * width + col + col_parity] as i32
                                - frame[(row + lag) * width + col + col_lag_parity] as i32,
                        );
                    }
                    let middle = diffs.len() / 2;
                    let (_, median, _) = diffs.select_nth_unstable(middle);
                    green_diffs[lag - 1][row] = *median as f32;
                    if row < 10 {
                        println!("row: {row:04}, lag: {lag:04}, green_diff: {median:04}")
                    }
                }
            }
        });


        let mut row_noise_removed =
            unsafe { self.context.get_uninit_cpu_buffer(interp.required_bytes()) };
        frame.storage.as_slice(|src| {
            let src: &[u16] = bytemuck::cast_slice(src);
            row_noise_removed.as_mut_slice(|dst| {
                let dst: &mut [u16] = bytemuck::cast_slice_mut(dst);
                for row in model.num_uncorrectable()..(height - model.num_uncorrectable()) {
                    let weights = if row % 2 == 0 { &model.weights_even } else { &model.weights_odd };
                    let mut offset = weights.offset;
                    offset += if row % 2 == 0 {
                        dark_col_mean_offset_even
                    } else {
                        dark_col_mean_offset_odd
                    };

                    for (lag, lag_weights) in weights.green_diff_weights.iter().enumerate() {
                        if row < 10 {
                            println!("row: {}, lag: {}, green diff: {} weight: {}", row, lag, green_diffs[lag][row - lag - 1], lag_weights[0]);
                            println!("row: {}, lag: {}, green diff: {} weight: {}", row, lag, green_diffs[lag][row], lag_weights[1]);
                        }
                        // the lag for the row is 1 based. lag == 0 means we want this offseted by one
                        offset -= green_diffs[lag][row - lag - 1] * lag_weights[0];
                        offset += green_diffs[lag][row] * lag_weights[1];
                    }


                    for (i, [weights_even, weights_odd]) in weights.dark_col_row_weights.iter().enumerate() {
                        // 0, 1, 2, 3, 4 -> 0, -1, 1, -2, 2
                        let i = i as isize;
                        let lag = (i / 2) - (i % 2);
                        let even_row = row - (row % 2);

                        for col in 0..NUM_DARKCOLS {
                            // if row == 2 {
                            //     dbg!(weights_even[col]);
                            //     dbg!(src[(even_row as isize + 2 * lag) * width + col] as f32 - BLACK_LEVEL);
                            //     dbg!(weights_even[col + NUM_DARKCOLS]);
                            //     dbg!(src[(even_row + 2 * lag) * width + (width - NUM_DARKCOLS) + col] as f32 - BLACK_LEVEL);
                            // }
                            offset += weights_even[col] * (src[(even_row as isize + 2 * lag) as usize * width + col] as f32 - BLACK_LEVEL);
                            offset += weights_even[col + NUM_DARKCOLS]
                                * (src[(even_row as isize + 2 * lag) as usize * width + (width - NUM_DARKCOLS) + col] as f32 - BLACK_LEVEL);
                        }

                        for col in 0..NUM_DARKCOLS {
                            offset +=
                                weights_odd[col] * (src[(even_row as isize + 1 + 2 * lag) as usize * width + col] as f32 - BLACK_LEVEL);
                            offset += weights_odd[col + NUM_DARKCOLS]
                                * (src[(even_row as isize + 1 + 2 * lag) as usize * width
                                    + (width - NUM_DARKCOLS)
                                    + col] as f32 - BLACK_LEVEL);
                        }
                    }

                    if row < 50 {
                        println!("row: {row:04}, offset: {offset:04.02}");
                    }
                    for col in NUM_DARKCOLS..(width - NUM_DARKCOLS) {
                        dst[row * width + col] = (src[row * width + col] as f32 - offset) as u16;
                    }
                }
            })
        });

        Ok(Payload::from(Frame { storage: row_noise_removed, interp }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
