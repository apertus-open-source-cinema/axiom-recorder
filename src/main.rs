#![feature(test)]
extern crate test;

fn main() {
    println!("Hello, world!");
}

pub fn rgb_to_rgba(src: &[u8], dst: &mut [u8]) {
    /*
        for i in 0..100 {

        }
    */

    /*
    for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = 255;
    }*/
}

pub fn rgb_to_chunks(src: &[u8], dst: &mut [u8]) {}

#[cfg(test)]
mod tests {
    use rand::Rng;
    use test::{black_box, Bencher};

    #[bench]
    fn bench_weird(b: &mut Bencher) {
        let mut rng = rand::thread_rng();
        let S = 4096 * 2160;
        let src: Vec<u8> = (0..(S * 3)).map(|_| rng.gen()).collect();
        let mut dst: Vec<u8> = vec![255; S * 4];

        b.iter(|| {
            /*
                            for (src, dst) in src.chunks_exact(3 * 8).zip(dst.chunks_exact_mut(4 * 8)) {

                        for i in 0..src.len()/3 {
                            unsafe {
                                let d = 4 * i;
                                let s = 3 * i;
                                dst.get_unchecked_mut(d..(d + 4)).copy_from_slice(src.get_unchecked(s..(s + 4)));
            //                    *dst.get_unchecked_mut(d + 3) = 255;
                            }
                        }
                            }
                        */

            for (src, dst) in src.chunks_exact(3 * 4).zip(dst.chunks_exact_mut(4 * 4)) {
                dst[0..4].copy_from_slice(&src[0..4]);
                dst[4..8].copy_from_slice(&src[3..7]);
                dst[8..12].copy_from_slice(&src[6..10]);
                dst[12..15].copy_from_slice(&src[9..12]);
                dst[3] = 255;
                dst[7] = 255;
                dst[11] = 255;
                dst[15] = 255;
            }

            black_box(&dst);
        });
    }

    #[bench]
    fn bench_plain(b: &mut Bencher) {
        let mut rng = rand::thread_rng();
        let S = 4096 * 2048;
        let src: Vec<u8> = (0..(S * 3)).map(|_| rng.gen()).collect();
        let mut dst: Vec<u8> = vec![255; S * 4];

        b.iter(|| {
            for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
                dst[3] = 255;
            }

            black_box(&dst);
        });
    }

    #[bench]
    fn bench_illegal(b: &mut Bencher) {
        let mut rng = rand::thread_rng();
        let S = 4096 * 2160;
        let src: Vec<u8> = (0..(S * 3)).map(|_| rng.gen()).collect();
        let mut dst: Vec<u8> = vec![255; S * 4];

        b.iter(|| {
            for (src, dst) in src.chunks_exact(3 * 4).zip(dst.chunks_exact_mut(4 * 4)) {
                for i in 0..src.len() / 3 {
                    unsafe {
                        let d = 4 * i;
                        let s = 3 * i;
                        dst.get_unchecked_mut(d..(d + 4))
                            .copy_from_slice(src.get_unchecked(s..(s + 4)));
                       *dst.get_unchecked_mut(d + 3) = 255;
                    }
                }
            }

            black_box(&dst);
        });
    }
}
