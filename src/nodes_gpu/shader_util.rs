use crate::pipeline_processing::frame::{
    ColorInterpretation,
    FrameInterpretation,
    SampleInterpretation,
};
use anyhow::{bail, Result};
use indoc::{formatdoc, indoc};
use shaderc::CompilationArtifact;



pub fn compile_shader(shader_code: &str) -> Result<CompilationArtifact> {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.add_macro_definition("dtype", Some("float"));
    options.add_macro_definition("dtype2", Some("vec2"));
    options.add_macro_definition("dtype3", Some("vec3"));
    options.add_macro_definition("dtype4", Some("vec4"));
    let spirv = compiler.compile_into_spirv(
        &shader_code,
        shaderc::ShaderKind::Compute,
        "shader.glsl",
        "main",
        Some(&options),
    )?;
    Ok(spirv)
}

pub fn generate_single_node_shader(
    node_shader: String,
    input_interpretation: FrameInterpretation,
    output_interpretation: FrameInterpretation,
) -> Result<String> {
    let mut shader_code = String::new();
    shader_code.push_str(indoc!(
        "
        #version 450
        #extension GL_EXT_shader_explicit_arithmetic_types: enable
        #extension GL_EXT_shader_explicit_arithmetic_types_int8: require
        #extension GL_EXT_shader_explicit_arithmetic_types_float16: require

        layout(local_size_x = 256, local_size_y = 4, local_size_z = 1) in;
    "
    ));
    shader_code.push_str(read_sample_function(input_interpretation.sample_interpretation)?);
    shader_code.push_str(write_sample_function(output_interpretation.sample_interpretation)?);
    shader_code.push_str(&formatdoc!(
        "
        #define IN_WIDTH {}
        #define IN_HEIGHT {}
        #define WIDTH {}
        #define HEIGHT {}
        ",
        input_interpretation.width,
        input_interpretation.height,
        output_interpretation.width,
        output_interpretation.height
    ));
    shader_code.push_str(&read_pixel_function(input_interpretation.color_interpretation)?);
    shader_code.push_str(&write_pixel_function(output_interpretation.color_interpretation)?);
    shader_code.push_str(&node_shader);
    shader_code.push_str(indoc!(
        "
        void main() {
            uvec2 pos = gl_GlobalInvocationID.xy;
            if (pos.x >= WIDTH || pos.y >= HEIGHT) return;
            write_pixel(pos, produce_pixel(pos));
        }
        "
    ));

    Ok(shader_code)
}

fn read_sample_function(si: SampleInterpretation) -> Result<&'static str> {
    match si {
        SampleInterpretation::UInt(bits) => match bits {
            8 => Ok(indoc!(
                "
                layout(set = 0, binding = 0) buffer readonly Source { uint8_t data[]; } source;

                dtype read_sample(uint i) {
                    return dtype(source.data[i]) / 255.0;
                }
                "
            )),
            12 => Ok(indoc!(
                "
                layout(set = 0, binding = 0) buffer readonly Source { uint8_t data[]; } source;


                dtype read_sample(uint i) {
                    uint source_idx = i / 2 * 3;

                    uint v;
                    if (i % 2 == 0) {
                        uint8_t a = source.data[source_idx + 0];
                        uint8_t b = source.data[source_idx + 1];
                        v = (a << 4) | (b & 0xf0);
                    } else {
                        uint8_t b = source.data[source_idx + 1];
                        uint8_t c = source.data[source_idx + 2];
                        v = ((b & 0x0f) << 8) | c;
                    }

                    return dtype(v) / 4095.0;
                }
                "
            )),
            16 => Ok(indoc!(
                "
                layout(set = 0, binding = 0) buffer readonly Source { uint16_t data[]; } source;

                dtype read_sample(uint i) {
                    return dtype(source.data[i]) / 65535.0;
                }
                "
            )),
            _ => bail!("bit depth {bits} is not implemented for input :("),
        },
        SampleInterpretation::FP16 => Ok(indoc!(
            "
            layout(set = 0, binding = 0) buffer readonly Source { float16_t data[]; } source;

            dtype read_sample(uint i) {
                return dtype(source.data[i]);
            }
            "
        )),
        SampleInterpretation::FP32 => Ok(indoc!(
            "
            layout(set = 0, binding = 0) buffer readonly Source { float data[]; } source;

            dtype read_sample(uint i) {
                return dtype(source.data[i]);
            }
            "
        )),
    }
}
fn write_sample_function(si: SampleInterpretation) -> Result<&'static str> {
    match si {
        SampleInterpretation::UInt(bits) => match bits {
            8 => Ok(indoc!(
                "
                layout(set = 0, binding = 1) buffer writeonly Sink { uint8_t data[]; } sink;

                void write_sample(uint i, dtype v) {
                    sink.data[i] = uint8_t(v * 255.0);
                }
                "
            )),
            16 => Ok(indoc!(
                "
                layout(set = 0, binding = 1) buffer writeonly Sink { uint16_t data[]; } sink;

                void write_sample(uint i, dtype v) {
                    sink.data[i] = uint16_t(v * 65535.0);
                }
                "
            )),
            _ => bail!("bit depth {bits} is not implemented for output :("),
        },
        SampleInterpretation::FP16 => Ok(indoc!(
            "
            layout(set = 0, binding = 1) buffer writeonly Sink { float16_t data[]; } sink;

            void write_sample(uint i, dtype v) {
                sink.data[i] = float16_t(v);
            }
            "
        )),
        SampleInterpretation::FP32 => Ok(indoc!(
            "
            layout(set = 0, binding = 1) buffer writeonly Sink { float data[]; } sink;

            void write_sample(uint i, dtype v) {
                sink.data[i] = float(v);
            }
            "
        )),
    }
}

fn read_pixel_function(ci: ColorInterpretation) -> Result<String> {
    match ci {
        ColorInterpretation::Bayer(cfa) => Ok(formatdoc!(
            "
            #define CFA_RED_IN_FIRST_ROW {}
            #define CFA_RED_IN_FIRST_COL {}

            dtype read_pixel(uvec2 pos) {{
                return read_sample(pos.y * IN_WIDTH + pos.x);
            }}
            ",
            cfa.red_in_first_row,
            cfa.red_in_first_col
        )),
        ColorInterpretation::Rgb => Ok(indoc!(
            "
            dtype3 read_pixel(uvec2 pos) {
                uint offset = pos.y * IN_WIDTH * 3 + pos.x * 3;
                dtype r = read_sample(offset + 0);
                dtype g = read_sample(offset + 1);
                dtype b = read_sample(offset + 2);
                return dtype3(r, g, b);
            }
            "
        )
        .to_string()),
        ColorInterpretation::Rgba => Ok(indoc!(
            "
            dtype4 read_pixel(uvec2 pos) {
                uint offset = pos.y * IN_WIDTH * 4 + pos.x * 4;
                dtype r = read_sample(offset + 0);
                dtype g = read_sample(offset + 1);
                dtype b = read_sample(offset + 2);
                dtype a = read_sample(offset + 3);
                return dtype3(r, g, b, a);
            }
            "
        )
        .to_string()),
    }
}
fn write_pixel_function(ci: ColorInterpretation) -> Result<String> {
    match ci {
        ColorInterpretation::Bayer(cfa) => Ok(formatdoc!(
            "
            #define OUT_CFA_RED_IN_FIRST_ROW {}
            #define OUT_CFA_RED_IN_FIRST_COL {}

            void write_pixel(uvec2 pos, dtype v) {{
                write_sample(pos.y * WIDTH + pos.x, v);
            }}
            ",
            cfa.red_in_first_row,
            cfa.red_in_first_col
        )),
        ColorInterpretation::Rgb => Ok(indoc!(
            "
            void write_pixel(uvec2 pos, dtype3 v) {
                uint offset = pos.y * WIDTH * 3 + pos.x * 3;
                write_sample(offset + 0, v.r);
                write_sample(offset + 1, v.g);
                write_sample(offset + 2, v.b);
            }
            "
        )
        .to_string()),
        ColorInterpretation::Rgba => Ok(indoc!(
            "
            void write_pixel(uvec2 pos, dtype4 v) {
                uint offset =  pos.y * WIDTH * 4 + pos.x * 4;
                write_sample(offset + 0, v.r);
                write_sample(offset + 1, v.g);
                write_sample(offset + 2, v.b);
                write_sample(offset + 3, v.a);
            }
            "
        )
        .to_string()),
    }
}
