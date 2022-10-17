use anyhow::bail;
use soapysdr::Direction::{Rx, Tx};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::anyhow::Result;

mod command;
mod config;
mod sink;
mod source;

pub use self::command::*;
pub use self::config::*;
pub use self::sink::*;
pub use self::source::*;

static SOAPY_INIT: async_lock::Mutex<()> = async_lock::Mutex::new(());

pub struct SoapyDevice<T> {
    dev: Option<soapysdr::Device>,
    init_cfg: Arc<Mutex<SoapyInitConfig>>,
    chans: Vec<usize>,
    stream: Option<T>,
}

// Note: there is additional impl in [`Self::command`]
impl<T> SoapyDevice<T> {
    pub fn apply_multi_config(
        &mut self,
        mcfg: &SoapyMultiConfig,
        default_dir: &SoapyConfigDir,
    ) -> Result<()> {
        for c in &mcfg.0 {
            self.apply_config(c, default_dir)?;
        }
        Ok(())
    }

    pub fn apply_config(&mut self, cfg: &SoapyConfig, default_dir: &SoapyConfigDir) -> Result<()> {
        let opt_dev = self.dev.clone();

        let dev = match opt_dev {
            None => {
                warn!("attempted apply_config without device");
                return Ok(()); //TODO: bail and catch elsewhere?
            }
            Some(d) => d,
        };

        let dir = match cfg.dir {
            SoapyConfigDir::Default => &default_dir,
            _ => &cfg.dir,
        };
        let dir_flags = match (dir.is_rx(), dir.is_tx()) {
            (true, true) => vec![Rx, Tx],
            (true, false) => vec![Rx],
            (false, true) => vec![Tx],
            _ => vec![],
        };

        debug!("apply_config: dirs: {:?}", dir_flags);

        let chans = match cfg.chan {
            Some(chan) => vec![chan],
            None => self.chans.clone(),
        };

        // Assume all channels in both directions use the same sample rate
        if let Some(rate) = cfg.sample_rate {
            debug!("set_sample_rate({})", rate);
            dev.set_sample_rate(Rx, 0, rate)?;
        }

        for dir in &dir_flags {
            for chan in &chans {
                if let Some(freq) = cfg.freq {
                    dev.set_frequency(*dir, *chan, freq, ())?;
                }
                // if let Some(rate) = cfg.sample_rate {
                //     debug!("set_sample_rate({:?},{},{})", *dir, *chan, rate);
                //     dev.set_sample_rate(*dir, *chan, rate)?;
                // }
                if let Some(gain) = cfg.gain {
                    dev.set_gain(*dir, *chan, gain)?;
                }
                if let Some(ref a) = cfg.antenna {
                    dev.set_antenna(*dir, *chan, a.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn apply_init_config(&mut self, default_dir: &SoapyConfigDir) -> Result<()> {
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
        self.apply_multi_config(&cfg.config, default_dir)?;
        self.chans = cfg.chans.clone();
        Ok(())
    }
}

unsafe impl<T> Sync for SoapyDevice<T> {}

pub struct SoapyDevBuilder<T> {
    init_cfg: SoapyInitConfig,
    cfg: SoapyConfig,
    _phantom: PhantomData<T>,
}

// TODO: need to allow different settings per channel
impl<T> SoapyDevBuilder<T> {
    /// For backwards compatibility. See: [`Self::device()`]
    #[deprecated]
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
        self.cfg.freq = Some(freq);
        self
    }

    /// See [`soapysdr::Device::set_sample_rate()`]
    pub fn sample_rate(mut self, sample_rate: f64) -> SoapyDevBuilder<T> {
        self.cfg.sample_rate = Some(sample_rate);
        self
    }

    /// See [`soapysdr::Device::set_gain()`]
    pub fn gain(mut self, gain: f64) -> SoapyDevBuilder<T> {
        self.cfg.gain = Some(gain);
        self
    }

    /// See [`soapysdr::Device::set_antenna()`]
    pub fn antenna<S>(mut self, antenna: S) -> SoapyDevBuilder<T>
    where
        S: Into<String>,
    {
        self.cfg.antenna = Some(antenna.into());
        self
    }
}
