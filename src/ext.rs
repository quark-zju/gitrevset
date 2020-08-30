use crate::Error;
use crate::Result;
use gitdag::dag::Set;
use gitdag::dag::Vertex;
use gitdag::git2::Oid;
use std::collections::HashMap;

pub trait OidExt {
    fn to_vertex(&self) -> Vertex;
}

impl OidExt for Oid {
    fn to_vertex(&self) -> Vertex {
        Vertex::copy_from(self.as_bytes())
    }
}

pub trait Merge {
    fn merge(&mut self, other: Self);
}

impl<K: std::cmp::Eq + std::hash::Hash, V> Merge for HashMap<K, V> {
    fn merge(&mut self, other: Self) {
        for (k, v) in other {
            self.insert(k, v);
        }
    }
}

/// Extended methods on `Set` struct.
pub trait SetExt {
    /// Convert to a convenient iterator of `Oid`s.
    fn to_oids(&self) -> Result<Box<dyn Iterator<Item = Result<Oid>>>>;
}

impl SetExt for Set {
    fn to_oids(&self) -> Result<Box<dyn Iterator<Item = Result<Oid>>>> {
        let iter = self.iter()?.map(|v| match v {
            Ok(v) => match Oid::from_bytes(v.as_ref()) {
                Ok(oid) => Ok(oid),
                Err(e) => Err(Error::from(e)),
            },
            Err(e) => Err(Error::from(e)),
        });
        Ok(Box::new(iter))
    }
}
