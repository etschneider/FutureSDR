use std::collections::HashMap;

use futuresdr::{
    anyhow::Result,
    async_io::block_on,
    blocks::{soapy::*, NullSink, Source},
    log::{debug, info},
    num_complex::Complex,
    runtime::{Flowgraph, Runtime},
};
use futuresdr_pmt::Pmt;

/// Example to illustrate the use of multiple Soapy channels on a single device
///
/// This is really only useful as a coding example and test case. It simply connects
/// soapy sources and sinks to null sinks and a constant source.

fn main() -> Result<()> {
    futuresdr::runtime::init(); //For logging

    let mut fg = Flowgraph::new();

    // Create a Soapy device to be shared by all the channels
    let soapy_dev = soapysdr::Device::new("driver=uhd")?;

    // Custom setup of the device can be done prior to handing it off to the FG.
    // E.g. A timed start is needed for multi-usrp/channel uhd rx
    let radio_time = soapy_dev.get_hardware_time(None)?;
    let start_time = radio_time + 3 * 1_000_000_000;
    debug!("radio_time: {}", radio_time);
    debug!("start_time: {}", start_time);

    let dev_spec = SoapyDevSpec::Dev(soapy_dev);

    let soapy_src_blk = SoapySourceBuilder::new()
        .device(dev_spec.clone())
        .channel(0)
        .channel(1)
        .freq(100e6)
        .sample_rate(1e6)
        .gain(0.0)
        .activate_time(start_time)
        .build();

    let soapy_snk_blk = SoapySinkBuilder::new()
        .device(dev_spec.clone())
        .channel(0)
        .channel(1)
        .freq(100e6)
        .sample_rate(1e6)
        .gain(0.0)
        .activate_time(start_time)
        .build();

    let soapy_src = fg.add_block(soapy_src_blk);
    let soapy_snk = fg.add_block(soapy_snk_blk);

    let zero_src = fg.add_block(Source::new(|| Complex::new(0.0f32, 0.0f32)));
    let null_snk1 = fg.add_block(NullSink::<Complex<f32>>::new());
    let null_snk2 = fg.add_block(NullSink::<Complex<f32>>::new());

    fg.connect_stream(soapy_src, "out", null_snk1, "in")?;
    fg.connect_stream(soapy_src, "out2", null_snk2, "in")?;
    fg.connect_stream(zero_src, "out", soapy_snk, "in")?;
    fg.connect_stream(zero_src, "out", soapy_snk, "in2")?;

    // This single line is really all that is needed for most cases:
    // Runtime::new().run(fg)?;

    // //////////////////
    // The following will use the flowgraph handle to send messages to the ports.
    // The flowgraph interface is inherently async, thus the use on the
    // block_on(async closure) pattern.
    let rt = Runtime::new();
    let (task, mut fg_handle) = block_on(rt.start(fg));

    // Pause for a few sec to wait for streaming to start (activate_time)
    info!("sleep...");
    std::thread::sleep(std::time::Duration::from_secs(4));

    // //////////////////
    // Use the old "freq" and "sample_rate" ports

    // Tune to different freq using the (old) "freq" port (port_id:0)
    info!("freq adjust");
    block_on(async {
        // We could use fg_handle.call() if we didn't want the return value
        let rv = fg_handle.callback(soapy_src, 0, Pmt::F64(101e6)).await;
        info!("retval: {:?}", rv);
    });

    // Adjust sample rate using the (old) "sample_rate" (port_id:1)
    // Note: Things can go wonky if we only do this on one port/direction.
    // info!("sample rate");
    // block_on(async {
    //     let rv = fg_handle.callback(soapy_src, 1, Pmt::F64(2e6)).await;
    //     info!("retval: {:?}", rv);
    // });

    // //////////////////
    // Use the 'cmd' port (port_id:2) a few different ways.

    // Like a GNU Radio Soapy block
    info!("cmd port: config via Pmt::MapStrPmt");
    block_on(async {
        let pmt = Pmt::MapStrPmt(HashMap::from([
            ("chan".to_owned(), Pmt::U32(0)),
            ("freq".to_owned(), Pmt::F64(102e6)),
            ("gain".to_owned(), Pmt::F32(1.0)),
        ]));
        let rv = fg_handle.callback(soapy_snk, 2, pmt).await;
        info!("retval: {:?}", rv);
    });

    use SoapyConfigItem as SCI;

    info!("cmd port: config via Pmt::Any(SoapyConfig)");
    block_on(async {
        let pmt = SoapyConfig::new()
            .push(SCI::Freq(90e6))
            .push(SCI::Gain(0.0))
            .to_pmt();
        let rv = fg_handle.callback(soapy_snk, 2, pmt).await;
        info!("retval: {:?}", rv);
    });

    info!("cmd port: config via Pmt::Any(SoapyConfig) w/ chan/dir spec");
    block_on(async {
        let pmt = SoapyConfig::new()
            .push(SCI::Channel(Some(0)))
            .push(SCI::Direction(SoapyDirection::Both))
            .push(SCI::Freq(91e6))
            .push(SCI::Gain(0.0))
            .push(SCI::Channel(Some(1)))
            .push(SCI::Direction(SoapyDirection::Both))
            .push(SCI::Freq(92e6))
            .push(SCI::Gain(0.0))
            .to_pmt();
        let rv = fg_handle.callback(soapy_snk, 2, pmt).await;
        info!("retval: {:?}", rv);
    });

    // //////////////////
    // Run a bit longer and then terminate
    block_on(async {
        info!("sleep...");
        futuresdr::async_io::Timer::after(std::time::Duration::from_secs(2)).await;
        info!("sending terminate");
        fg_handle.terminate().await.unwrap();
        let _ = task.await;
    });

    info!("The End.");
    Ok(())
}
