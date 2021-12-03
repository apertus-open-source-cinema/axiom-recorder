use std::sync::Arc;
use recorder::nodes_io::reader_raw::RawDirectoryReader;
use recorder::nodes_io::writer_cinema_dng::CinemaDngWriter;
use recorder::pipeline_processing::parametrizable::{Parameterizable, Parameters, ParameterValue};
use recorder::pipeline_processing::processing_context::ProcessingContext;
use recorder::pipeline_processing::node::ProcessingSink;

pub fn main() {
    let context = ProcessingContext::default();
    let input = Arc::new(RawDirectoryReader::from_parameters(
        &Parameters([
            ("file-pattern".to_owned(), ParameterValue::StringParameter("test/bubbles/*".to_owned())),
            ("fps".to_owned(), ParameterValue::FloatRange(24.0)),
            ("width".to_owned(), ParameterValue::IntRange(3840)),
            ("height".to_owned(), ParameterValue::IntRange(2160)),
            ("bit-depth".to_owned(), ParameterValue::IntRange(12)),
            ("first-red-x".to_owned(), ParameterValue::BoolParameter(false)),
            ("first-red-y".to_owned(), ParameterValue::BoolParameter(true)),
            ("loop".to_owned(), ParameterValue::BoolParameter(true))
        ].into_iter().cloned().collect())).unwrap());
    let output = CinemaDngWriter::from_parameters(
        &Parameters([
        ("input".to_owned(), ParameterValue::NodeInput(input)),
        ("path".to_owned(), ParameterValue::StringParameter("new_rt_test".to_owned())),
    ].into_iter().cloned().collect())).unwrap();

    pollster::block_on(output.run(context));
}