use anyhow::{bail, Result};
use futuresdr_pmt::Pmt;
use serde::{Deserialize, Serialize};
use std::fmt;

// TODO: This should be supported by the Pmt library directly
pub fn pmt_to_f64(pmt: &Pmt) -> Result<f64> {
    let v = match pmt {
        Pmt::F64(v) => *v,
        Pmt::F32(v) => *v as f64,
        Pmt::U32(v) => *v as f64,
        Pmt::U64(v) => *v as f64,
        _ => bail!("can't convert PMT to f64"),
    };
    Ok(v)
}

// TODO: This should be supported by the Pmt library directly
pub fn pmt_to_usize(pmt: &Pmt) -> Result<usize> {
    let v = match pmt {
        Pmt::F64(v) => *v as usize,
        Pmt::F32(v) => *v as usize,
        Pmt::U32(v) => *v as usize,
        Pmt::U64(v) => *v as usize,
        _ => bail!("can't convert PMT to usize"),
    };
    Ok(v)
}

/// Soapy device specifier options
#[derive(Clone, Serialize, Deserialize)]
pub enum SoapyDevSpec {
    Filter(String),
    #[serde(skip)]
    Dev(soapysdr::Device),
}

impl Default for SoapyDevSpec {
    fn default() -> Self {
        Self::Filter("".to_owned())
    }
}

impl fmt::Debug for SoapyDevSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoapyDevSpec::Filter(s) => write!(f, "Filter({}", s),
            SoapyDevSpec::Dev(_) => write!(f, "Dev"), //TODO: retrieve some ID info
        }
    }
}

/// Specify the channel direction to which a configuration applies.
///
/// There are scenarios where a custom command processor may need
/// access to both Tx and Rx direction simultaneously to perform its
/// task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoapyConfigDir {
    /// Use the direction of the block being configured.
    ///
    /// [`SoapySource`]: `Rx`
    /// [`SoapySink`]: `Tx`
    Default,
    Rx,
    Tx,
    Both,
}

impl SoapyConfigDir {
    pub fn is_rx(&self) -> bool {
        match self {
            SoapyConfigDir::Rx => true,
            SoapyConfigDir::Both => true,
            _ => false,
        }
    }

    pub fn is_tx(&self) -> bool {
        match self {
            SoapyConfigDir::Tx => true,
            SoapyConfigDir::Both => true,
            _ => false,
        }
    }
}

impl Default for SoapyConfigDir {
    fn default() -> Self {
        Self::Default
    }
}

/// Runtime configuration settings
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SoapyConfig {
    /// Specifies the channel that the the configuration apply to.
    ///
    /// If None, the settings will be applied to all channels.
    pub chan: Option<usize>,

    /// Channel direction of settings
    ///
    /// Normally a source or sink block will set this internally if the default value
    /// of [`SoapyConfigDir::Default`] is set.  
    pub dir: SoapyConfigDir,

    /// See [`soapysdr::Device::set_sample_rate()`]
    pub sample_rate: Option<f64>,

    /// See [`soapysdr::Device::set_frequency()`]
    pub freq: Option<f64>,

    /// See [`soapysdr::Device::set_gain()`]
    pub gain: Option<f64>,

    /// See [`soapysdr::Device::set_antenna()`]
    pub antenna: Option<String>,

    /// See [`soapysdr::Device::set_bandwidth()`]
    pub bandwidth: Option<f64>,
    //
    // And many more....
}

/// Convert a Pmt into a [`SoapyConfig`] type.
///
/// [`Pmt::Any(SoapyConfig)`]: This simply downcasts and thus exposes all supported
/// configuration options. This is the preferred type.
///
/// [`Pmt::MapStrPmt`]: this roughly mirrors the `cmd` port dict of the GNU Radio
/// [Soapy](https://wiki.gnuradio.org/index.php/Soapy) block. Only a subset of the
/// possible configuration items will be available to this type.
impl TryFrom<Pmt> for SoapyConfig {
    type Error = anyhow::Error;

    fn try_from(pmt: Pmt) -> Result<Self, Self::Error> {
        match pmt {
            Pmt::Any(a) => {
                if let Some(cfg) = a.downcast_ref::<Self>() {
                    Ok(cfg.clone())
                } else {
                    bail!("downcast failed")
                }
            }
            Pmt::MapStrPmt(m) => {
                let mut cfg = Self::default();
                for (n, v) in m.iter() {
                    match (n.as_str(), v) {
                        // We could add entries to support multiple Pmt types per name,
                        // but it is probably best to make the accepted type correspond
                        // to the actual SoapySDR API.
                        ("antenna", Pmt::String(v)) => cfg.antenna = Some(v.to_owned()),
                        ("bandwidth", p) => cfg.bandwidth = Some(pmt_to_f64(&p)?),
                        ("chan", p) => cfg.chan = Some(pmt_to_usize(&p)?),
                        ("freq", p) => cfg.freq = Some(pmt_to_f64(&p)?),
                        ("gain", p) => cfg.gain = Some(pmt_to_f64(&p)?),
                        ("rate", p) => cfg.sample_rate = Some(pmt_to_f64(&p)?),
                        // By default, log a warning but otherwise ignore
                        _ => warn!("unrecognized name/value pair: {}", n),
                    }
                }
                Ok(cfg)
            }
            _ => bail!("cannot convert this PMT"),
        }
    }
}

/// Multiple [`SoapyConfig`] structs.
///
/// A [`SoapyConfig`] struct only represent settings for a single channel type.
/// This type allows for different settings for different channels.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SoapyMultiConfig(pub Vec<SoapyConfig>);

/// Convert a Pmt into a [`SoapyMultiConfig`] type.
///
/// [`Pmt::Any(SoapyMultiConfig)`]: This simply downcasts and thus exposes all supported
/// configuration options. This is the preferred type.
///
/// [`Pmt::VecPmt`]: this attempt to convert each element using [`SoapyConfig::try_from(Pmt)`].
impl TryFrom<Pmt> for SoapyMultiConfig {
    type Error = anyhow::Error;

    fn try_from(pmt: Pmt) -> Result<Self, Self::Error> {
        match pmt {
            Pmt::Any(a) => {
                if let Some(mcfg) = a.downcast_ref::<Self>() {
                    Ok(mcfg.clone())
                } else if let Some(cfg) = a.downcast_ref::<SoapyConfig>() {
                    //Accept a single SoapyConfig as a convenience
                    Ok(Self(vec![cfg.clone()]))
                } else {
                    bail!("downcast failed")
                }
            }
            Pmt::VecPmt(v) => {
                let mut mcfg = Self::default();
                for p in v {
                    match SoapyConfig::try_from(p) {
                        Ok(cfg) => mcfg.0.push(cfg.clone()),
                        Err(e) => bail!(e),
                    }
                }
                Ok(mcfg)
            }
            Pmt::MapStrPmt(_) => {
                //Accept a single SoapyConfig as a convenience
                let cfg = SoapyConfig::try_from(pmt)?;
                Ok(Self(vec![cfg]))
            }
            // Pmt::Any(a) => {}
            _ => bail!("cannot convert this PMT"),
        }
    }
}

/// Initialization only configuration items
///
/// These items can only used during initialization, not while the device is
/// streaming.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SoapyInitConfig {
    pub dev: SoapyDevSpec,

    /// Which hardware channels to use.
    pub chans: Vec<usize>,

    /// Set the stream activation time.
    ///
    /// The value should be relative to the value returned from
    /// [`soapysdr::Device::get_hardware_time()`]    
    pub activate_time: Option<i64>,

    /// Initial values of runtime modifiable settings.
    pub config: SoapyMultiConfig,
}
