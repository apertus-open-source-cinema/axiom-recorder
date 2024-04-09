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
    fn num_green_lags(&self) -> usize { self.green_diff_weights.len() }

    fn num_dark_cols(&self) -> usize { 1 + self.dark_col_row_weights.len() / 2 }
}

impl RowNoiseRemovalModel {
    fn num_green_lags(&self) -> usize { self.weights_even.num_green_lags() }

    fn num_dark_cols(&self) -> usize { self.weights_even.num_dark_cols() }

    fn num_uncorrectable(&self) -> usize {
        return self.num_green_lags().max((self.num_dark_cols() - 1) * 2);
    }
}

pub struct RowNoiseRemoval {
    model: RowNoiseRemovalModel,
    input: InputProcessingNode,
    context: ProcessingContext,
    strip_dark_columns: bool,
}

impl Parameterizable for RowNoiseRemoval {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("strip-dark-columns", Optional(BoolParameter))
            .with("model", WithDefault(StringParameter, StringValue("internal:good".to_owned())))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self> {
        let model_path: String = parameters.take("model")?;
        let model_yml = match model_path.as_str() {
            "internal:good" => include_str!("./good.yml").to_owned(),
            "internal:only_dark" => include_str!("./only_dark.yml").to_owned(),
            "internal:only_green" => include_str!("./only_green.yml").to_owned(),
            "internal:mean" => include_str!("./mean.yml").to_owned(),
            _ => std::fs::read_to_string(&model_path)
                .with_context(|| format!("Failed to read model from {}", model_path))?,
        };

        Ok(Self {
            input: parameters.take("input")?,
            strip_dark_columns: parameters.take("strip-dark-columns")?,
            context: context.clone(),
            model: serde_yaml::from_str(&model_yml)?,
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

        let model = &self.model;

        let mut green_diffs =
            vec![vec![0f32; (height - model.num_uncorrectable())]; model.num_green_lags()];

        let slice = frame.storage.as_slice(|frame| {
            let frame: &[u16] = bytemuck::cast_slice(frame);

            for row in 0..(height - model.num_uncorrectable()) {
                for lag in 0..model.num_green_lags() {
                    let lag = lag + 1;
                    let mut diffs = Vec::new();

                    // a random bayer pattern for thinking support:
                    // |  B  |  G  |  B  |  G  |
                    // |  G  |  R  |  G  |  R  |
                    // |  B  |  G  |  B  |  G  |
                    // |  G  |  R  |  G  |  R  |

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
                }
            }
        });


        let strip_offset = if self.strip_dark_columns { NUM_DARKCOLS } else { 0 };
        let output_width = (interp.width - 2 * (strip_offset as u64)) as usize;
        let output_interp = Raw { width: output_width as u64, ..interp };
        let mut row_noise_removed =
            unsafe { self.context.get_uninit_cpu_buffer(output_interp.required_bytes()) };

        frame.storage.as_slice(|src| {
            let src: &[u16] = bytemuck::cast_slice(src);
            row_noise_removed.as_mut_slice(|dst| {
                let dst: &mut [u16] = bytemuck::cast_slice_mut(dst);
                for row in model.num_uncorrectable()..(height - model.num_uncorrectable()) {
                    let weights =
                        if row % 2 == 0 { &model.weights_even } else { &model.weights_odd };
                    let mut offset = weights.offset;

                    for (lag, lag_weights) in weights.green_diff_weights.iter().enumerate() {
                        // the lag for the row is 1 based. lag == 0 means we want this offseted by
                        // one
                        offset -= green_diffs[lag][row - lag - 1] * lag_weights[0];
                        offset += green_diffs[lag][row] * lag_weights[1];
                    }


                    for (i, [weights_even, weights_odd]) in
                        weights.dark_col_row_weights.iter().enumerate()
                    {
                        // 0, 1, 2, 3, 4 -> 0, -1, 1, -2, 2
                        let i = i as isize;
                        let lag = (i / 2) - (i % 2);
                        let even_row = row - (row % 2);

                        for col in 0..NUM_DARKCOLS {
                            offset += weights_even[col]
                                * (src[(even_row as isize + 2 * lag) as usize * width + col]
                                    as f32
                                    - BLACK_LEVEL);
                            offset += weights_even[col + NUM_DARKCOLS]
                                * (src[(even_row as isize + 2 * lag) as usize * width
                                    + (width - NUM_DARKCOLS)
                                    + col] as f32
                                    - BLACK_LEVEL);
                        }

                        for col in 0..NUM_DARKCOLS {
                            offset += weights_odd[col]
                                * (src[(even_row as isize + 1 + 2 * lag) as usize * width + col]
                                    as f32
                                    - BLACK_LEVEL);
                            offset += weights_odd[col + NUM_DARKCOLS]
                                * (src[(even_row as isize + 1 + 2 * lag) as usize * width
                                    + (width - NUM_DARKCOLS)
                                    + col] as f32
                                    - BLACK_LEVEL);
                        }
                    }

                    for col in 0..output_width {
                        dst[row * output_width + col] =
                            (src[row * width + col + strip_offset] as f32 - offset) as u16;
                    }
                }


                // fill the uncorrectable rows with the original pixel data
                // TODO(robin): maybe fall back to a simpler model without any uncorrectable
                // rows?
                for row in 0..model.num_uncorrectable() {
                    for col in 0..output_width {
                        dst[row * output_width + col] = src[row * width + col + strip_offset];
                    }
                }

                for row in (height - model.num_uncorrectable())..height {
                    for col in 0..output_width {
                        dst[row * output_width + col] = src[row * width + col + strip_offset];
                    }
                }
            })
        });

        Ok(Payload::from(Frame { storage: row_noise_removed, interp: output_interp }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
