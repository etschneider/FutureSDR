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
pub enum SoapyDirection {
    /// Use the direction of the block being configured.
    ///
    /// [`SoapySource`]: `Rx`
    /// [`SoapySink`]: `Tx`
    Default,
    Rx,
    Tx,
    Both,
    None,
}

impl SoapyDirection {
    pub fn is_rx(&self, default: &Self) -> bool {
        match self {
            Self::Default => default.is_rx(&Self::None),
            Self::Rx => true,
            Self::Both => true,
            _ => false,
        }
    }

    pub fn is_tx(&self, default: &Self) -> bool {
        match self {
            Self::Default => default.is_tx(&Self::None),
            Self::Tx => true,
            Self::Both => true,
            _ => false,
        }
    }
}

impl Default for SoapyDirection {
    fn default() -> Self {
        Self::Default
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoapyConfigItem {
    Direction(SoapyDirection),
    /// Channel(None) applies to all enabled channels
    Channel(Option<usize>),
    Antenna(String),
    Bandwidth(f64),
    Freq(f64),
    Gain(f64),
    SampleRate(f64),
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SoapyConfig(pub Vec<SoapyConfigItem>);

impl SoapyConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, ci: SoapyConfigItem) -> &mut Self {
        self.0.push(ci);
        self
    }

    pub fn to_pmt(&self) -> Pmt {
        Pmt::Any(Box::new(self.clone()))
    }
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
        use SoapyConfigItem as SCI;

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
                        ("antenna", Pmt::String(v)) => {
                            cfg.push(SCI::Antenna(v.to_owned()));
                        }
                        ("bandwidth", p) => {
                            cfg.push(SCI::Bandwidth(pmt_to_f64(&p)?));
                        }
                        ("chan", p) => {
                            cfg.push(SCI::Channel(Some(pmt_to_usize(&p)?)));
                        }
                        ("freq", p) => {
                            cfg.push(SCI::Freq(pmt_to_f64(&p)?));
                        }
                        ("gain", p) => {
                            cfg.push(SCI::Gain(pmt_to_f64(&p)?));
                        }
                        ("rate", p) => {
                            cfg.push(SCI::SampleRate(pmt_to_f64(&p)?));
                        }
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
    pub config: SoapyConfig,
}
