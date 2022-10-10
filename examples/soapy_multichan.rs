use async_io::block_on;
use axum::extract::Extension;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_macros::debug_handler;
use futuresdr_pmt::Pmt;
use log::debug;
use num_complex::Complex;
use std::sync::Arc;
use std::sync::Mutex;

use futuresdr::anyhow::Result;
use futuresdr::blocks::NullSink;
use futuresdr::blocks::SoapySinkBuilder;
use futuresdr::blocks::SoapySourceBuilder;
use futuresdr::blocks::Source;
use futuresdr::runtime::Flowgraph;
use futuresdr::runtime::FlowgraphHandle;
use futuresdr::runtime::Runtime;

/// Example to illustrate the use of multiple Soapy channels on a single device
///
/// This is really only useful as a coding example. It simply connects
/// soapy sources and sinks to null sinks and a constant source.

fn main() -> Result<()> {
    futuresdr::runtime::init(); //For logging

    let mut fg = Flowgraph::new();
    let arc_opt_fg_handle: Arc<Mutex<Option<FlowgraphHandle>>> = Arc::new(Mutex::new(None));

    // Create a Soapy device to be shared by all the channels
    let soapy_dev = soapysdr::Device::new("driver=uhd")?;

    // Custom setup of the device can be done prior to handing it off to the FG.
    // E.g. A timed start is needed for multi-usrp/channel uhd rx
    let radio_time = soapy_dev.get_hardware_time(None)?;
    let start_time = radio_time + 3 * 1_000_000_000;
    debug!("radio_time: {}", radio_time);
    debug!("start_time: {}", start_time);

    let soapy_src = SoapySourceBuilder::new()
        .device(soapy_dev.clone())
        .channel(0)
        .channel(1)
        .freq(100e6)
        .sample_rate(1e6)
        .gain(0.0)
        .activate_time(start_time)
        .build();

    let soapy_snk = SoapySinkBuilder::new()
        .device(soapy_dev.clone())
        .channel(0)
        .channel(1)
        .freq(100e6)
        .sample_rate(1e6)
        .gain(0.0)
        .activate_time(start_time)
        .build();

    let soapy_src = fg.add_block(soapy_src);
    let soapy_snk = fg.add_block(soapy_snk);

    let zero_src = fg.add_block(Source::new(|| Complex::new(0.0f32, 0.0f32)));
    let null_snk1 = fg.add_block(NullSink::<Complex<f32>>::new());
    let null_snk2 = fg.add_block(NullSink::<Complex<f32>>::new());

    fg.connect_stream(soapy_src, "out1", null_snk1, "in")?;
    fg.connect_stream(soapy_src, "out2", null_snk2, "in")?;
    fg.connect_stream(zero_src, "out", soapy_snk, "in1")?;
    fg.connect_stream(zero_src, "out", soapy_snk, "in2")?;

    let router = Router::new()
        .route("/my_route/", get(my_route))
        .layer(Extension((arc_opt_fg_handle.clone(), soapy_dev.clone())));

    fg.set_custom_routes(router);

    println!("Visit http://127.0.0.1:1337/my_route/");
    let (task_handle, fgh) = block_on(Runtime::new().start(fg));
    {
        let mut opt_fg_handle = arc_opt_fg_handle.lock().unwrap();
        *opt_fg_handle = Some(fgh.clone());
    }
    block_on(task_handle)?;

    // Runtime::new().run(fg)?;
    Ok(())
}

#[debug_handler]
async fn my_route(
    Extension((arc_opt_fgh, dev)): Extension<(
        Arc<Mutex<Option<FlowgraphHandle>>>,
        soapysdr::Device,
    )>,
) -> Html<String> {
    //Note: need to clone out of mutext guard, which is not `Send`, which
    //breaks handler due to use of .await (Axum thing)
    let mut opt_fgh = arc_opt_fgh.lock().unwrap().clone();

    let dstr = if let Some(ref mut fgh) = opt_fgh {
        format!("{:#?}", fgh.description().await.unwrap())
        // format!("Test")
    } else {
        "".to_owned()
    };

    //access soapy dev
    let freq = dev.frequency(soapysdr::Direction::Rx, 0).unwrap();
    let fr = dev.frequency_range(soapysdr::Direction::Rx, 0).unwrap();

    //Call FG
    if let Some(ref mut fgh) = opt_fgh {
        fgh.call(0, 0, Pmt::F64(200e6)).await.unwrap();
    }

    Html(format!(
        r#"
    <html>
        <head>
            <meta charset='utf-8' />
            <title>FutureSDR</title>
        </head>
        <body>
            <h2>Current freq:</h2> 
            <pre>{}</pre>
            <h2>Soapy freq range:</h2> 
            <pre>{:?}</pre>
            <h1>My Custom Route</h1>
            <h2>Flowgraph description:</h2> 
            <pre>{}</pre>
        </body>
    </html>
    "#,
        freq, fr, dstr
    ))
}
