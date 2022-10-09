use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;

mod description;
pub use description::BlockDescription;
pub use description::FlowgraphDescription;

pub trait PmtAny: Any + DynClone + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
dyn_clone::clone_trait_object!(PmtAny);

impl<T: Any + DynClone + Send + Sync + 'static> PmtAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl dyn PmtAny {
    pub fn downcast_ref<T: PmtAny>(&self) -> Option<&T> {
        (*self).as_any().downcast_ref::<T>()
    }
    pub fn downcast_mut<T: PmtAny>(&mut self) -> Option<&mut T> {
        (*self).as_any_mut().downcast_mut::<T>()
    }
}

impl fmt::Debug for Box<dyn PmtAny> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Box<dyn Any>")
    }
}

/// A serializable version of [PmtAny]
///
/// This functionality is provided by the typetag crate.
///
/// Unfortunately, typetag (v0.2) does not support automatic deserialization
/// traits via `#[typetag::serde]` on generic impls, so we need to impl the
/// `Any` functions for every type that will be used with [Pmt::AnySerde].
///
/// E.g.:
/// ```no_run
/// struct MyStruct(u32);
///
/// #[typetag::serde]
/// impl PmtAnySerde for MyStruct {
///     fn as_any(&self) -> &dyn Any {
///         self
///     }
///     fn as_any_mut(&mut self) -> &mut dyn Any {
///         self
///     }
/// }
/// ```
#[typetag::serde(tag = "type")]
pub trait PmtAnySerde: Any + DynClone + Send + Sync + std::fmt::Debug + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
dyn_clone::clone_trait_object!(PmtAnySerde);

impl dyn PmtAnySerde {
    pub fn downcast_ref<T: PmtAnySerde>(&self) -> Option<&T> {
        (*self).as_any().downcast_ref::<T>()
    }
    pub fn downcast_mut<T: PmtAnySerde>(&mut self) -> Option<&mut T> {
        (*self).as_any_mut().downcast_mut::<T>()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pmt {
    Null,
    String(String),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    VecF32(Vec<f32>),
    VecU64(Vec<u64>),
    Blob(Vec<u8>),
    VecPmt(Vec<Pmt>),
    MapStrPmt(HashMap<String, Pmt>),
    #[serde(skip)]
    Any(Box<dyn PmtAny>),
    AnySerde(Box<dyn PmtAnySerde>),
}

impl PartialEq for Pmt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Pmt::Null, Pmt::Null) => true,
            (Pmt::String(x), Pmt::String(y)) => x == y,
            (Pmt::U32(x), Pmt::U32(y)) => x == y,
            (Pmt::U64(x), Pmt::U64(y)) => x == y,
            (Pmt::F32(x), Pmt::F32(y)) => x == y,
            (Pmt::F64(x), Pmt::F64(y)) => x == y,
            (Pmt::VecF32(x), Pmt::VecF32(y)) => x == y,
            (Pmt::VecU64(x), Pmt::VecU64(y)) => x == y,
            (Pmt::Blob(x), Pmt::Blob(y)) => x == y,
            (Pmt::VecPmt(x), Pmt::VecPmt(y)) => x == y,
            (Pmt::MapStrPmt(x), Pmt::MapStrPmt(y)) => x == y,
            //How to handle Any?
            _ => false,
        }
    }
}

impl Pmt {
    pub fn is_string(&self) -> bool {
        matches!(self, Pmt::String(_))
    }

    pub fn to_string(&self) -> Option<String> {
        match &self {
            Pmt::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn from_string(s: &str, t: &PmtKind) -> Option<Pmt> {
        match t {
            PmtKind::U32 => {
                if let Ok(v) = s.parse::<u32>() {
                    Some(Pmt::U32(v))
                } else {
                    None
                }
            }
            PmtKind::U64 => {
                if let Ok(v) = s.parse::<u64>() {
                    Some(Pmt::U64(v))
                } else {
                    None
                }
            }
            PmtKind::F32 => {
                if let Ok(v) = s.parse::<f32>() {
                    Some(Pmt::F32(v))
                } else {
                    None
                }
            }
            PmtKind::F64 => {
                if let Ok(v) = s.parse::<f64>() {
                    Some(Pmt::F64(v))
                } else {
                    None
                }
            }
            PmtKind::String => Some(Pmt::String(s.to_string())),
            _ => None,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, PartialEq, Eq)]
pub enum PmtKind {
    Null,
    String,
    U32,
    U64,
    F32,
    F64,
    VecF32,
    VecU64,
    Blob,
    VecPmt,
    MapStrPmt,
    Any,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pmt() {
        let p = Pmt::Null;
        assert!(!p.is_string());
        assert_eq!(p.to_string(), None);
        let p = Pmt::String("foo".to_owned());
        assert!(p.is_string());
        assert_eq!(p.to_string(), Some("foo".to_owned()));
    }

    #[test]
    fn pmt_serde() {
        let p = Pmt::Null;
        let mut s = flexbuffers::FlexbufferSerializer::new();
        p.serialize(&mut s).unwrap();

        let r = flexbuffers::Reader::get_root(s.view()).unwrap();
        let p2 = Pmt::deserialize(r).unwrap();

        assert_eq!(p, p2);
    }

    #[test]
    fn pmt_any_serde() {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct TestStruct {
            a: u32,
            b: f64,
            c: (String, u32),
        }

        // Unfortunately, typetag does not support #[typetag::serde] on generic impls,
        // so we need to impl the any functions for every instance.
        #[typetag::serde]
        impl PmtAnySerde for TestStruct {
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }

        let st_pre = TestStruct {
            a: 1,
            b: 2.0,
            c: ("Three".to_owned(), 3),
        };
        let pmt_any = Pmt::AnySerde(Box::new(st_pre.clone()));

        let s = serde_json::to_string(&pmt_any).unwrap();
        // println!("{}", s);
        let de = serde_json::from_str(&s).unwrap();

        match de {
            Pmt::AnySerde(de_any) => {
                if let Some(st_de) = de_any.downcast_ref::<TestStruct>() {
                    assert_eq!(st_pre, *st_de);
                } else {
                    panic!("downcast failed");
                }
            }
            _ => panic!("not Pmt::AnySerde"),
        }
    }

    #[allow(clippy::many_single_char_names)]
    #[test]
    fn pmt_eq() {
        let a = Pmt::Null;
        let b = Pmt::U32(123);
        assert_ne!(a, b);

        let c = Pmt::Null;
        let d = Pmt::U32(12);
        let e = Pmt::U32(123);
        assert_eq!(a, c);
        assert_eq!(b, e);
        assert_ne!(b, d);

        let f1 = Pmt::F32(0.1);
        let f2 = Pmt::F32(0.1);
        let f3 = Pmt::F32(0.2);
        assert_eq!(f1, f2);
        assert_ne!(f1, f3);

        // How to handle this?
        // let pa = Pmt::Any(Box::new((1, 2, 3)));
        // assert_eq!(pa, pa);
    }

    #[test]
    fn vec_pmt() {
        let vpmt = Pmt::VecPmt(vec![Pmt::U32(1), Pmt::U32(2)]);

        if let Pmt::VecPmt(v) = vpmt {
            assert_eq!(v[0], Pmt::U32(1));
            assert_eq!(v[1], Pmt::U32(2));
        } else {
            panic!("Not a Pmt::VecPmt");
        }
    }

    #[test]
    fn map_str_pmt() {
        let u32val = 42;
        let f64val = 6.02214076e23;

        let msp = Pmt::MapStrPmt(HashMap::from([
            ("str".to_owned(), Pmt::String("a string".to_owned())),
            (
                "submap".to_owned(),
                Pmt::MapStrPmt(HashMap::from([
                    ("U32".to_owned(), Pmt::U32(u32val)),
                    ("F64".to_owned(), Pmt::F64(f64val)),
                ])),
            ),
        ]));

        if let Pmt::MapStrPmt(m) = msp {
            if let Some(Pmt::MapStrPmt(sm)) = m.get("submap") {
                assert_eq!(sm.get("U32"), Some(&Pmt::U32(u32val)));
                assert_eq!(sm.get("F64"), Some(&Pmt::F64(f64val)));
            } else {
                panic!("Could not get submap");
            }
        } else {
            panic!("Not a Pmt::MapStrPmt");
        }
    }
}
