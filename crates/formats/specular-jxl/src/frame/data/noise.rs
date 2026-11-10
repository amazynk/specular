#[derive(Debug)]
pub struct NoiseParameters {
    pub lut: [f32; 8],
}

impl<Ctx> crate::common::Bundle<Ctx> for NoiseParameters {
    type Error = crate::frame::Error;

    fn parse(bitstream: &mut crate::bitstream::Bitstream, _: Ctx) -> crate::frame::Result<Self> {
        let mut lut = [0.0f32; 8];
        for slot in &mut lut {
            *slot = bitstream.read_bits(10)? as f32 / (1 << 10) as f32;
        }

        Ok(Self { lut })
    }
}
