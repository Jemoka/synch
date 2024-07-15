use crdts::{CmRDT};
use crdts::list::{Op, List};
use serde::{Serialize, Deserialize, Serializer};
use serde::ser::{SerializeStruct};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::fmt::Debug;

type PhantomUnsend = PhantomData<std::sync::MutexGuard<'static, ()>>;

#[derive(Deserialize, Debug)]
pub struct SyncedList<T: Clone> {
    list: List<T, usize>,
    actor: usize,
    #[serde(skip)] 
    tape: Vec<Op<T, usize>>,
    
}

pub struct SyncedListGuard<'a, T: Clone> {
    value: T,
    idx: usize,
    src: &'a mut SyncedList<T>,
    was_mutated: bool,
    _not_send: PhantomUnsend,
}

impl<'a, T: Clone + Debug> Debug for SyncedListGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncedListGuard")
            .field("value", &self.value)
            .field("index", &self.idx)
            .field("was_mutated", &self.was_mutated)
            .finish()
    }
}

impl<T:Clone> Deref for SyncedListGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T:Clone> DerefMut for SyncedListGuard<'_, T> {
    fn deref_mut (&mut self) -> &mut Self::Target {
        self.was_mutated = true;
        &mut self.value
    }
}

impl<T:Clone> Drop for SyncedListGuard<'_, T> {
    fn drop (&mut self)  {
        if self.was_mutated {
            let delete_op = self.src.list.delete_index(self.idx, self.src.actor)
                .expect(
                    &format!("index out of bounds: length is {} but index is {}",
                             self.src.len(), self.idx)
                );
            self.src.list.apply(delete_op.clone());
            self.src.tape.push(delete_op);

            let insert_op = self.src.list.insert_index(self.idx, self.value.clone(),
                                                       self.src.actor);
            self.src.list.apply(insert_op.clone());
            self.src.tape.push(insert_op);

        }
    }
}



impl<T: Clone + Serialize> Serialize for SyncedList<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut pack = serializer.serialize_struct("SyncedList", 2)?;
        pack.serialize_field("list", &self.list)?;
        pack.serialize_field("actor", &(self.actor + 1))?;
        pack.end()
    }
}

impl<T: Clone> SyncedList<T> {
    pub fn new() -> Self {
        SyncedList { list: List::new(), actor: 0, tape: vec![] } 
    }

    /// Get the length of the list.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Get an element from the list, optionally setting it.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut list:SyncedList<u32> = SyncedList::new():
    /// list.push(1);
    /// assert_eq!(*list.get(0).unwrap(), 1);
    /// *list.get(0).unwrap() = 2;
    /// assert_eq!(*list.get(0).unwrap(), 2);
    /// ```
    pub fn get(&mut self, idx: usize) -> Option<SyncedListGuard<T>> {
        if self.len() > idx {
            Some(SyncedListGuard {
                value: self.list.position(idx).unwrap().clone(),
                idx: idx,
                src: self,
                was_mutated: false,
                _not_send: PhantomData
            })
        } else { None }
    }

    /// Push an element to the list.
    pub fn push(&mut self, element: T) {
        self.apply(self.list.append(element, self.actor));
    }

    /// Remove an element from the list.
    pub fn remove(&mut self, index: usize) {
        self.apply(self.list.delete_index(index, self.actor).expect(
            &format!("index out of bounds: length is {} but index is {}",
                     self.len(), index)
        ));
    }

    /// Insert an element into the list.
    pub fn insert(&mut self, index: usize, element: T) {
        self.apply(self.list.insert_index(index, element, self.actor));
    }

    fn apply(&mut self, op: Op<T, usize>) {
        self.list.apply(op.clone());
        self.tape.push(op);
    }

    /// Replay the tape of the list, removing its tape.
    ///
    /// # Note 
    /// If the tape is not published onto the wire, it  will be lost forever
    /// and not recoverable. 
    pub fn replay(&mut self) -> Vec<Op<T, usize>> {
        let mut old_tape = vec![];
        std::mem::swap(&mut self.tape, &mut old_tape);
        old_tape
    }
}

impl<T: Clone> Clone for SyncedList<T> {
    fn clone(&self) -> Self {
        SyncedList { list: self.list.clone(), actor: self.actor + 1, tape: vec![] }
    }
}

impl<T: Clone> Into<Vec<T>> for SyncedList<T> {
    fn into(self) -> Vec<T> {
        self.list.read_into::<Vec<_>>()
    }
}

impl<T: Clone> IntoIterator for SyncedList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.list.read_into::<Vec<_>>().into_iter()
    }
}
