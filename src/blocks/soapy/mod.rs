use anyhow::bail;
use serde::{Deserialize, Serialize};
use soapysdr::Direction::{Rx, Tx};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::anyhow::{Context, Result};
use crate::runtime::Pmt;

mod config;
mod sink;
mod source;

pub use self::config::*;
pub use self::sink::*;
pub use self::source::*;

static SOAPY_INIT: async_lock::Mutex<()> = async_lock::Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoapyCommand {
    // /// Set the device initialization data
    // ///
    // /// It will be applied on the next init()
    // InitConfig(SoapyInitConfig),
    /// Apply configuration
    Config(SoapyConfig),
    /// Retrieve the device configuration
    GetConfig(),
    // /// A user defined type (requires a custom command handler)
    // User(Pmt::AnySerde),
}

pub struct SoapyDevice<T> {
    dev: Option<soapysdr::Device>,
    init_cfg: Arc<Mutex<SoapyInitConfig>>,
    chans: Vec<usize>,
    stream: Option<T>,
}

// Note: there is additional impl in [`Self::command`]
impl<T> SoapyDevice<T> {
    /// The handler for messages on the "cmd" port.
    ///
    /// [`default_dir`]: A default direction that is set by the block
    /// to indicate if it is a source or sink. This is only a default, some
    /// messages may specify different directions, regardless of the natural
    /// direction of the block.
    ///
    pub fn base_cmd_handler(&mut self, pmt: Pmt, default_dir: &SoapyDirection) -> Result<Pmt> {
        if let Pmt::Any(a) = &pmt {
            if let Some(cmd) = a.downcast_ref::<SoapyCommand>() {
                match cmd {
                    SoapyCommand::Config(c) => {
                        self.apply_config(c, default_dir)?;
                        return Ok(Pmt::Null);
                    }
                    _ => bail!("unimplemented"),
                };
            }
        }

        // If not a command just try as a configuration type
        match SoapyConfig::try_from(pmt) {
            Ok(cfg) => {
                self.apply_config(&cfg, default_dir)?;
                return Ok(Pmt::Null);
            }
            Err(e) => bail!(e),
        }
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    pub fn set_freq(&mut self, p: Pmt, default_dir: &SoapyDirection) -> Result<Pmt> {
        let dev = self.dev.as_mut().context("no dev")?;

        let freq = pmt_to_f64(&p)?;

        if default_dir.is_rx(&SoapyDirection::None) {
            dev.set_frequency(Rx, 0, freq, ())?;
        }
        if default_dir.is_tx(&SoapyDirection::None) {
            dev.set_frequency(Tx, 0, freq, ())?;
        }
        Ok(Pmt::Null)
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    pub fn set_sample_rate(&mut self, p: Pmt, default_dir: &SoapyDirection) -> Result<Pmt> {
        let dev = self.dev.as_mut().context("no dev")?;

        let rate = pmt_to_f64(&p)?;

        if default_dir.is_rx(&SoapyDirection::None) {
            dev.set_sample_rate(Rx, 0, rate)?;
        }
        if default_dir.is_tx(&SoapyDirection::None) {
            dev.set_sample_rate(Tx, 0, rate)?;
        }
        Ok(Pmt::Null)
    }

    pub fn apply_config(&mut self, cfg: &SoapyConfig, default_dir: &SoapyDirection) -> Result<()> {
        use SoapyConfigItem as SCI;

        let opt_dev = self.dev.clone();

        let dev = match opt_dev {
            None => {
                warn!("attempted apply_config without device");
                return Ok(()); //TODO: bail and catch elsewhere?
            }
            Some(d) => d,
        };

        let mut chans = self.chans.clone();

        let update_dir = |d: &SoapyDirection| -> Vec<soapysdr::Direction> {
            match (d.is_rx(&default_dir), d.is_tx(&default_dir)) {
                (false, true) => vec![Tx],
                (true, false) => vec![Rx],
                (true, true) => vec![Rx, Tx],
                _ => vec![],
            }
        };

        let mut dir_flags = update_dir(default_dir);

        debug!("initial dir:{:?} chans:{:?})", dir_flags, chans);

        for ci in &cfg.0 {
            match ci {
                SCI::Antenna(a) => {
                    for d in dir_flags.iter() {
                        for c in chans.iter() {
                            dev.set_antenna(*d, *c, a.as_bytes())?;
                        }
                    }
                }
                SCI::Bandwidth(bw) => {
                    for d in dir_flags.iter() {
                        for c in chans.iter() {
                            dev.set_bandwidth(*d, *c, *bw)?;
                        }
                    }
                }
                SCI::Channel(chan) => {
                    chans = match chan {
                        Some(chan) => vec![*chan],
                        None => self.chans.clone(),
                    };
                }
                SCI::Direction(d) => {
                    dir_flags = update_dir(d);
                }
                SCI::Freq(freq) => {
                    for d in dir_flags.iter() {
                        for c in chans.iter() {
                            debug!("dev.set_frequency({:?},{},{})", *d, *c, *freq);
                            dev.set_frequency(*d, *c, *freq, ())?;
                        }
                    }
                }
                SCI::Gain(gain) => {
                    for d in dir_flags.iter() {
                        for c in chans.iter() {
                            debug!("dev.set_gain({:?},{},{})", *d, *c, *gain);
                            dev.set_gain(*d, *c, *gain)?;
                        }
                    }
                }
                SCI::SampleRate(rate) => {
                    for d in dir_flags.iter() {
                        for c in chans.iter() {
                            debug!("dev.set_sample_rate({:?},{},{})", *d, *c, *rate);
                            dev.set_sample_rate(*d, *c, *rate)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn apply_init_config(&mut self, default_dir: &SoapyDirection) -> Result<()> {
        let cfg_mtx = &self.init_cfg.clone();
        let cfg = cfg_mtx.lock().unwrap();

        match &cfg.dev {
            SoapyDevSpec::Dev(d) => {
                self.dev = Some(d.clone());
            }
            SoapyDevSpec::Filter(f) => {
                let dev = soapysdr::Device::new(f.as_str());
                match dev {
                    Ok(d) => {
                        self.dev = Some(d);
                    }
                    Err(e) => {
                        bail!("Soapy device init error: {}", e);
                    }
                };
            }
        };
        self.chans = cfg.chans.clone();
        self.apply_config(&cfg.config, default_dir)?;
        Ok(())
    }
}

// unsafe impl<T> Sync for SoapyDevice<T> {}

pub struct SoapyDevBuilder<T> {
    init_cfg: SoapyInitConfig,
    _phantom: PhantomData<T>,
}

// TODO: need to allow different settings per channel
impl<T> SoapyDevBuilder<T> {
    /// Specify a device using a filter string.
    ///
    /// See [`Self::device()`] for a more flexible option.
    pub fn filter(mut self, filter: &str) -> SoapyDevBuilder<T> {
        self.init_cfg.dev = SoapyDevSpec::Filter(filter.to_string());
        self
    }

    /// Specify the soapy device.
    ///
    /// See: [`SoapyDevSpec`] and [`soapysdr::Device::new()`]
    pub fn device(mut self, dev_spec: SoapyDevSpec) -> SoapyDevBuilder<T> {
        self.init_cfg.dev = dev_spec;
        self
    }

    /// Set the stream activation time.
    ///
    /// The value should be relative to the value returned from
    /// [`soapysdr::Device::get_hardware_time()`]
    pub fn activate_time(mut self, time_ns: i64) -> SoapyDevBuilder<T> {
        self.init_cfg.activate_time = Some(time_ns);
        self
    }

    /// Add a channel.
    ///
    /// This can be applied multiple times.
    pub fn channel(mut self, chan: usize) -> SoapyDevBuilder<T> {
        self.init_cfg.chans.push(chan);
        self
    }

    /// See [`soapysdr::Device::set_frequency()`]
    pub fn freq(mut self, freq: f64) -> SoapyDevBuilder<T> {
        self.init_cfg.config.push(SoapyConfigItem::Freq(freq));
        self
    }

    /// See [`soapysdr::Device::set_sample_rate()`]
    pub fn sample_rate(mut self, sample_rate: f64) -> SoapyDevBuilder<T> {
        self.init_cfg
            .config
            .push(SoapyConfigItem::SampleRate(sample_rate));
        self
    }

    /// See [`soapysdr::Device::set_gain()`]
    pub fn gain(mut self, gain: f64) -> SoapyDevBuilder<T> {
        self.init_cfg.config.push(SoapyConfigItem::Gain(gain));
        self
    }

    /// See [`soapysdr::Device::set_antenna()`]
    pub fn antenna<S>(mut self, antenna: S) -> SoapyDevBuilder<T>
    where
        S: Into<String>,
    {
        self.init_cfg
            .config
            .push(SoapyConfigItem::Antenna(antenna.into()));
        self
    }
}
