pub mod decoder {

    ///mod digital_decoder;
use digital_decoder::decoder::DigitalDecoder;

    const HW_RATIO: i32 = 17;

    const MIN_OOK_THRESHOLD: f32 = 0.25;
    const OOK_THRESHOLD_RATIO: f32 = 0.75;
    const OOK_DECAY_PER_SAMPLE: f32 = 0.0001;

    const FILTER_ALPHA: f32 = 0.7;

    pub struct AnalogDecoder <'a>{
        pub discarded_samples: i32,
        pub ook_max: f32,
        pub val: f32,
        pub cb: Box<Fn( &mut  DigitalDecoder <'a>, u8)>,
        //pub cb: Option<fn(u8)>,
    }

    impl <'a> AnalogDecoder<'a> {
        pub fn new() -> AnalogDecoder<'a>{
     AnalogDecoder{
         discarded_samples: 0,
         ook_max: 0.0,
         val: 0.0,
         cb: Box::new( DigitalDecoder::handle_data),
    }
        }

        pub fn handle_magnitude(&mut self, value: f32) {
            //
            // Smooth
            //
            self.val = (FILTER_ALPHA) * self.val + (1.0 - FILTER_ALPHA) * value;
            let mut val = self.val;

            //
            // 1 of N
            //
            if self.discarded_samples < (HW_RATIO - 1) {
                self.discarded_samples += 1;
                return;
            }

            self.discarded_samples = 0;

            //
            // Saturate
            //
            val = val.min(1.0);

            //
            // Threshold
            //
            self.ook_max -= OOK_DECAY_PER_SAMPLE;
            self.ook_max = self.ook_max.max(val);
            self.ook_max = self.ook_max.max(MIN_OOK_THRESHOLD / OOK_THRESHOLD_RATIO);

            //
            // Send to digital stage
            //
            // if self.cb {
            //     if val > self.ook_max * OOK_THRESHOLD_RATIO {
            //         self.cb(1);
            //     } else {
            //         self.cb(0);
            //     }
            // }
        }

        // setCallback(std::function<void(char)> cb) {m_cb = cb;}{}
        // std::function<void(char)> m_cb;
    }
}
