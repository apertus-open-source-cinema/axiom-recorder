#[inline(never)]
pub fn convert_12_to_8(input: &[u8], output: &mut [u8]) {
    for (input, output) in input.chunks_exact(3).zip(output.chunks_exact_mut(2)) {
        output[0] = input[0];
        output[1] = (input[1] << 4) | (input[2] >> 4);
    }
}
