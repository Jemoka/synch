use crdts::ctx::ReadCtx;
use crdts::{CmRDT, MVReg};
use crdts::map::{Map, Op};
use std::cmp::{Ord, PartialEq};
use std::default::Default;
use std::ops::{Deref, DerefMut};
use std::fmt::Debug;

use std::marker::PhantomData;

type PhantomUnsend = PhantomData<std::sync::MutexGuard<'static, ()>>;

use super::taped::Taped;

pub trait MapKey: Clone + Ord + Debug {}
impl<T:?Sized + Clone + Ord + Debug> MapKey for T {}

pub trait MapVal: Clone + PartialEq + Default + Debug {}
impl<T:?Sized + Clone + PartialEq + Default + Debug> MapVal for T {}

pub struct SyncedMapElementGuard<'a, K: MapKey, V: MapVal> {
    key: Option<K>,
    ctx: Option<ReadCtx<Option<MVReg<V, usize>>, usize>>,
    value: V,
    src: &'a mut SyncedMap<K, V>,
    was_mutated: bool,
    _not_send: PhantomUnsend,
}

impl<K: MapKey, V: MapVal> Debug for SyncedMapElementGuard<'_, K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncedMapElementGuard")
            .field("key", self.key.as_ref().unwrap())
            .field("ctx", &self.ctx.as_ref().unwrap().val)
            .field("value", &self.value)
            .field("was_mutated", &self.was_mutated)
            .finish()
    }
}

impl<K: MapKey, V: MapVal> Deref for SyncedMapElementGuard<'_, K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<K: MapKey, V: MapVal> DerefMut for SyncedMapElementGuard<'_, K, V> {
    fn deref_mut (&mut self) -> &mut Self::Target {
        self.was_mutated = true;
        &mut self.value
    }
}

impl<K: MapKey, V: MapVal> Drop for SyncedMapElementGuard<'_, K, V> {
    fn drop (&mut self)  {
        if self.was_mutated {
            let mut read_context = None;
            std::mem::swap(&mut read_context, &mut self.ctx);
            let add_ctx = read_context.unwrap().derive_add_ctx(self.src.actor);

            let mut dropped_key = None;
            std::mem::swap(&mut dropped_key, &mut self.key);

            let mut dropped_value = V::default();
            std::mem::swap(&mut dropped_value, &mut self.value);

            let op = self.src.map.update(dropped_key.unwrap(), add_ctx, |v,a| v.write(dropped_value, a));
            self.src.map.apply(op.clone());
            self.src.tape.push(op);
        }
    }
}

/// Map Structure for Syncronized Operations
pub struct SyncedMap<K: MapKey, V: MapVal> {
    map: Map<K, MVReg<V, usize>, usize>,
    actor: usize,
    // #[serde(skip)] 
    tape: Vec<Op<K, MVReg<V, usize>, usize>>,
}

impl<K: MapKey, V: MapVal> Taped<usize> for SyncedMap<K, V> {
    type Operation = Op<K, MVReg<V, usize>, usize>;

    /// Synchronize your list against a tape
    fn replay(&mut self, tape: Vec<Self::Operation>) {
        tape.into_iter().for_each(|x| self.map.apply(x));
    }

    /// Grab the tape of the list, removing its tape.
    ///
    /// # Note 
    /// If the tape is not published onto the wire, it will be lost forever
    /// and not recoverable. 
    fn tape(&mut self) -> Vec<Self::Operation> {
        let mut old_tape = vec![];
        std::mem::swap(&mut self.tape, &mut old_tape);
        old_tape
    }
}

impl<'a, K: MapKey, V: MapVal> SyncedMap<K, V> {
    pub fn new() -> Self {
        SyncedMap {
            map: Map::new(),
            actor: 0,
            tape: vec![]
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        self.map.get(key)
            .val.and_then(|x| 
                          x.read().val.first()
                          .and_then(|y| Some(y.clone())))
    }
}

impl<K: MapKey, V: MapVal> Clone for SyncedMap<K, V> {
    fn clone(&self) -> Self {
        SyncedMap {
            map: self.map.clone(),
            actor: self.actor + 1,
            tape: vec![]
        }
    }
}
