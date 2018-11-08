//use analog_decoder::analog_decoder;
mod analog_decoder;
use analog_decoder::decoder::AnalogDecoder;

mod digital_decoder;
use digital_decoder::decoder::DigitalDecoder;

extern crate rtlsdr;
use rtlsdr::*;
use std::process::exit;
fn main() -> Result<(), RTLSDRError> {
    let mut mag_lut = vec![0 as f32, 0x10000 as f32];

    let mut d_decoder = DigitalDecoder::new();
    let mut a_decoder = AnalogDecoder::new();

    // Open the device
    //
    if rtlsdr::get_device_count() < 1 {
        println!("Could not find any devices");
        exit(-1);
    }

    let mut dev = rtlsdr::open(0)?;

    //
    // Set the frequency
    //
    rtlsdr::RTLSDRDevice::set_center_freq(&mut dev, 345_000_000)?;
    println!(
        "Successfully set the frequency to {}",
        rtlsdr::RTLSDRDevice::get_center_freq(&mut dev)?
    );

    // Set manual gain mode
    //
    rtlsdr::RTLSDRDevice::set_tuner_gain_mode(&mut dev, true)?;

    // Set the gain
    //
    rtlsdr::RTLSDRDevice::set_tuner_gain(&mut dev, 350)?;
    println!(
        "Successfully set gain to {}",
        rtlsdr::RTLSDRDevice::get_tuner_gain(&mut dev)
    );

    // Set the sample rate
    //
    rtlsdr::RTLSDRDevice::set_sample_rate(&mut dev, 1_000_000)?;
    println!(
        "Successfully set the sample rate to {}",
        rtlsdr::RTLSDRDevice::get_sample_rate(&mut dev)?
    );

    // Prepare for streaming
    //
    rtlsdr::RTLSDRDevice::reset_buffer(&mut dev)?;

    for ii in 0..0x10000 {
        let real_i = ii & 0xFF;
        let imag_i = ii >> 8;

        let real = ((real_i as f32) - 127.4) * (1.0 / 128.0);
        let imag = ((imag_i as f32) - 127.4) * (1.0 / 128.0);

        let mag = (real * real + imag * imag).sqrt();
        mag_lut[ii] = mag;
    }

    //
    // Common Receive
    //

    //
    //    //
    //    // Async Receive
    //    //
    //
    //    typedef void(*rtlsdr_read_async_cb_t)(unsigned char *buf, uint32_t len, void *ctx);
    //
    //    auto cb = [](unsigned char *buf, uint32_t len, void *ctx)
    //    {
    //        AnalogDecoder *adec = (AnalogDecoder *)ctx;
    //
    //        int n_samples = len/2;
    //        for i in 0..n_samples
    //        {
    //            float mag = magLut[*((uint16_t*)(buf + i*2))];
    //            adec->handleMagnitude(mag);
    //        }
    //    };
    //
    //    // Setup watchdog to check for a common-mode failure (e.g. antenna disconnection)
    //    std::signal(SIGALRM, alarmHandler);
    //
    //    // Initialize RX state to good
    //    dDecoder.setRxGood(true);
    //    const int err = rtlsdr_read_async(dev, cb, &aDecoder, 0, 0);
    //    println!( "Read Async returned " << err );

    /*    
    //
    // Synchronous Receive
    //
    static const size_t BUF_SIZE = 1024*256;
    uint8_t buffer[BUF_SIZE];
    
    while(true)
    {
        int n_read = 0;
        if(rtlsdr_read_sync(dev, buffer, BUF_SIZE, &n_read) < 0)
        {
            println!( "Failed to read from device" );
            return -1;
        }
        
        int n_samples = n_read/2;
        for(int i = 0; i < n_samples; ++i)
        {
            float mag = magLut[*((uint16_t*)(buffer + i*2))];
            aDecoder.handleMagnitude(mag);
        }
    }
*/

    // Shut down
    //
    rtlsdr::RTLSDRDevice::close(&mut dev);
    Ok(())
}
