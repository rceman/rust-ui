use std::any::{Any, TypeId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Public key trait: any `Eq + Hash + Clone + 'static` value is a key.
/// Keys are parent-local; identity uses TypeId + Eq, never the hash alone.
pub trait KeyId: Eq + Hash + Clone + 'static {}
impl<T: Eq + Hash + Clone + 'static> KeyId for T {}

/// Type-erased key. `hash` is a lookup hint only — identity is decided by
/// the `Any` equality check inside the same `TypeId` namespace.
#[derive(Clone)]
pub(crate) struct ErasedKey {
    type_id: TypeId,
    hash: u64,
    value: Rc<dyn ErasedKeyValue>,
}

pub(crate) trait ErasedKeyValue {
    fn as_any(&self) -> &dyn Any;
    fn eq_key(&self, other: &dyn ErasedKeyValue) -> bool;
}

struct KeyBox<K: KeyId>(K);

impl<K: KeyId> ErasedKeyValue for KeyBox<K> {
    fn as_any(&self) -> &dyn Any {
        &self.0
    }
    fn eq_key(&self, other: &dyn ErasedKeyValue) -> bool {
        other
            .as_any()
            .downcast_ref::<K>()
            .is_some_and(|k| *k == self.0)
    }
}

impl ErasedKey {
    pub(crate) fn new<K: KeyId>(key: K) -> Self {
        let mut h = DefaultHasher::new();
        key.hash(&mut h);
        ErasedKey {
            type_id: TypeId::of::<K>(),
            hash: h.finish(),
            value: Rc::new(KeyBox(key)),
        }
    }

    /// Cheap fingerprint for diagnostics — never carries key contents.
    pub(crate) fn fingerprint(&self) -> u64 {
        self.hash
    }

    /// Hint bucket for hash lookups; equality is decided by `key_eq`.
    pub(crate) fn hash(&self) -> u64 {
        self.hash
    }

    /// True identity check: same key type and equal value.
    pub(crate) fn key_eq(&self, other: &ErasedKey) -> bool {
        self.type_id == other.type_id && self.value.eq_key(&*other.value)
    }
}

/// Child identity inside one parent scope: an explicit erased key, or the
/// static fallback of (kind discriminant, ordinal among same-kind statics).
#[derive(Clone)]
pub(crate) enum ChildKey {
    Static { kind: u8, ordinal: u32 },
    Explicit(ErasedKey),
}

impl PartialEq for ErasedKey {
    fn eq(&self, other: &Self) -> bool {
        self.key_eq(other)
    }
}
impl Eq for ErasedKey {}
impl Hash for ErasedKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the type id too: equality requires same TypeId, so equal
        // keys always hash equal and unequal types spread apart
        self.type_id.hash(state);
        self.hash.hash(state);
    }
}

impl ChildKey {
    pub(crate) fn hash(&self) -> u64 {
        match self {
            ChildKey::Static { kind, ordinal } => {
                let mut h = DefaultHasher::new();
                kind.hash(&mut h);
                ordinal.hash(&mut h);
                h.finish()
            }
            ChildKey::Explicit(k) => k.hash(),
        }
    }

    pub(crate) fn key_eq(&self, other: &ChildKey) -> bool {
        match (self, other) {
            (
                ChildKey::Static {
                    kind: a,
                    ordinal: o,
                },
                ChildKey::Static {
                    kind: b,
                    ordinal: p,
                },
            ) => a == b && o == p,
            (ChildKey::Explicit(a), ChildKey::Explicit(b)) => a.key_eq(b),
            _ => false,
        }
    }
}
