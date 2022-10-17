use serde::{Deserialize, Serialize};
use soapysdr::Direction::{Rx, Tx};

use crate::anyhow::{Context, Result};
use crate::runtime::Pmt;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoapyCommand {
    /// Perform device initialization
    InitConfig(SoapyInitConfig),
    /// Process a single runtime configuration
    Config(SoapyConfig),
    /// Process multiple runtime configuration items at once
    MultiConfig(SoapyMultiConfig),
    /// Retrieve the device configuration
    GetConfig(),
    // /// A user defined type (requires a custom command handler)
    // User(Pmt::AnySerde),
}

impl<T> SoapyDevice<T> {
    /// The handler for messages on the "cmd" port.
    ///
    /// [`default_dir`]: A default direction that is set by the receiving block
    /// to indicate if it is a source or sink. This is only a default, some
    /// messages may specify different directions, regardless of the natural
    /// direction of the receiving block.
    ///
    pub fn base_cmd_handler(&mut self, pmt: Pmt, default_dir: &SoapyConfigDir) -> Result<Pmt> {
        if let Pmt::Any(a) = &pmt {
            if let Some(cmd) = a.downcast_ref::<SoapyCommand>() {
                match cmd {
                    SoapyCommand::Config(c) => {
                        self.apply_config(c, default_dir)?;
                        return Ok(Pmt::Null);
                    }
                    SoapyCommand::MultiConfig(c) => {
                        self.apply_multi_config(c, default_dir)?;
                        return Ok(Pmt::Null);
                    }
                    _ => bail!("unimplented"),
                };
            }
        }

        // If not a command just try as configuration
        match SoapyMultiConfig::try_from(pmt) {
            Ok(mcfg) => {
                self.apply_multi_config(&mcfg, default_dir)?;
                return Ok(Pmt::Null);
            }
            Err(e) => bail!(e),
        }
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    pub fn set_freq(&mut self, p: Pmt, default_dir: &SoapyConfigDir) -> Result<Pmt> {
        let dev = self.dev.as_mut().context("no dev")?;

        let freq = pmt_to_f64(&p)?;

        let is_rx = default_dir.is_rx();
        let is_tx = default_dir.is_tx();

        if is_rx {
            dev.set_frequency(Rx, 0, freq, ())?;
        }
        if is_tx {
            dev.set_frequency(Tx, 0, freq, ())?;
        }
        Ok(Pmt::Null)
    }

    // For backwards compatibility, can only set channel 0
    // #[deprecated]
    pub fn set_sample_rate(&mut self, p: Pmt, default_dir: &SoapyConfigDir) -> Result<Pmt> {
        let dev = self.dev.as_mut().context("no dev")?;

        let rate = pmt_to_f64(&p)?;

        let is_rx = default_dir.is_rx();
        let is_tx = default_dir.is_tx();

        if is_rx {
            dev.set_sample_rate(Rx, 0, rate)?;
        }
        if is_tx {
            dev.set_sample_rate(Tx, 0, rate)?;
        }
        Ok(Pmt::Null)
    }
}
