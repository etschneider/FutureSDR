use futures::{Future, FutureExt};
use std::cmp;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::{
    anyhow::{Context, Result},
    num_complex::Complex32,
    runtime::{
        Block, BlockMeta, BlockMetaBuilder, Kernel, MessageIo, MessageIoBuilder, Pmt, StreamIo,
        StreamIoBuilder, WorkIo,
    },
};

use super::*;

pub type SoapySource = SoapyDevice<soapysdr::RxStream<Complex32>>;

impl SoapySource {
    fn new(init_cfg: SoapyInitConfig) -> Block {
        let mut chans = init_cfg.chans.clone();
        if chans.is_empty() {
            chans.push(0);
        }

        let mut siob = StreamIoBuilder::new();

        for i in 0..chans.len() {
            if i == 0 {
                // Never number the first output port for compatibility with single port instances
                siob = siob.add_output::<Complex32>("out");
            } else {
                siob = siob.add_output::<Complex32>(&format!("out{}", i + 1));
            }
        }

        Block::new(
            BlockMetaBuilder::new("SoapySource").blocking().build(),
            siob.build(),
            MessageIoBuilder::new()
                .add_input("freq", Self::on_freq_port)
                .add_input("sample_rate", Self::on_sample_rate_port)
                .add_input("cmd", Self::on_cmd_port)
                .build(),
            SoapySource {
                dev: None,
                init_cfg: Arc::new(Mutex::new(init_cfg)),
                chans,
                stream: None,
            },
        )
    }

    fn on_cmd_port<'a>(
        &'a mut self,
        _mio: &'a mut MessageIo<Self>,
        _meta: &'a mut BlockMeta,
        p: Pmt,
    ) -> Pin<Box<dyn Future<Output = Result<Pmt>> + Send + 'a>> {
        async move { self.base_cmd_handler(p, &SoapyDirection::Rx) }.boxed()
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    fn on_freq_port<'a>(
        &'a mut self,
        _mio: &'a mut MessageIo<Self>,
        _meta: &'a mut BlockMeta,
        p: Pmt,
    ) -> Pin<Box<dyn Future<Output = Result<Pmt>> + Send + 'a>> {
        async move { self.set_freq(p, &SoapyDirection::Rx) }.boxed()
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    fn on_sample_rate_port<'a>(
        &'a mut self,
        _mio: &'a mut MessageIo<Self>,
        _meta: &'a mut BlockMeta,
        p: Pmt,
    ) -> Pin<Box<dyn Future<Output = Result<Pmt>> + Send + 'a>> {
        async move { self.set_sample_rate(p, &SoapyDirection::Rx) }.boxed()
    }
}

#[doc(hidden)]
#[async_trait]
impl Kernel for SoapySource {
    async fn work(
        &mut self,
        io: &mut WorkIo,
        sio: &mut StreamIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
    ) -> Result<()> {
        let outs = sio.outputs_mut();
        let bufs: Vec<&mut [Complex32]> = outs.iter_mut().map(|b| b.slice::<Complex32>()).collect();

        let min_out_len = bufs.iter().map(|b| b.len()).min().unwrap_or(0);

        let stream = self.stream.as_mut().unwrap();
        let n = cmp::min(min_out_len, stream.mtu().unwrap());
        if n == 0 {
            return Ok(());
        }

        if let Ok(len) = stream.read(&bufs, 1_000_000) {
            for i in 0..outs.len() {
                sio.output(i).produce(len);
            }
        }
        io.call_again = true;
        Ok(())
    }

    async fn init(
        &mut self,
        _sio: &mut StreamIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
    ) -> Result<()> {
        let _ = super::SOAPY_INIT.lock();
        soapysdr::configure_logging();
        if let Err(e) = self.apply_init_config(&SoapyDirection::Rx) {
            warn!("SoapySource::new() apply_init_config error: {}", e);
        }

        let dev = self.dev.as_ref().context("no dev")?;
        let cfg_mtx = &self.init_cfg.clone();
        let cfg = cfg_mtx.lock().unwrap();

        self.stream = Some(dev.rx_stream::<Complex32>(&self.chans)?);
        self.stream
            .as_mut()
            .context("no stream")?
            .activate(cfg.activate_time)?;

        Ok(())
    }

    async fn deinit(
        &mut self,
        _sio: &mut StreamIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
    ) -> Result<()> {
        self.stream
            .as_mut()
            .context("no stream")?
            .deactivate(None)?;
        Ok(())
    }
}

/// Build a [SoapySource].
///
/// Most logic is implemented in the shared [`SoapyDevBuilder`].
///
/// # Inputs
///
/// **Message** `freq`: a Pmt::u32 to change the frequency to.
///
/// # Outputs
///
/// `out`: Samples received from device.
///
/// # Usage
/// ```no_run
/// use futuresdr::blocks::SoapySourceBuilder;
/// use futuresdr::runtime::Flowgraph;
///
/// let mut fg = Flowgraph::new();
///
/// let source = fg.add_block(
///     SoapySourceBuilder::new()
///         .freq(100e9)
///         .sample_rate(1e6)
///         .gain(10.0)
///         .filter("device=hackrf")
///         .build()
/// );
/// ```
pub type SoapySourceBuilder = SoapyDevBuilder<SoapySource>;

impl SoapyDevBuilder<SoapySource> {
    pub fn new() -> Self {
        Self {
            init_cfg: SoapyInitConfig::default(),
            _phantom: PhantomData,
        }
    }

    pub fn build(self) -> Block {
        SoapySource::new(self.init_cfg)
    }
}
