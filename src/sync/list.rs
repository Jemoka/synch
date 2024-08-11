use crdts::{CmRDT};
use crdts::list::{Op, List};
use serde::{Serialize, Deserialize, Serializer};
use serde::ser::{SerializeStruct};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::fmt::Debug;

use super::taped::Taped;

type PhantomUnsend = PhantomData<std::sync::MutexGuard<'static, ()>>;

/// List Structure for Syncronized Operations
///
/// # Key Note
/// **Lists can only be synced if they are `.clone()` of each other.**
///
/// # Examples
///
/// ```
/// let mut amy:SyncedList<u8> = SyncedList::new();
/// let mut bob:SyncedList<u8> = amy.clone();
/// 
/// // push some values
/// amy.push(5);
/// bob.push(8);
/// // get some values
/// let first = amy.lock(0).expect("no values are here!")
/// // we can even change it
/// *first = 8;
/// // we can now syncronize the lists
/// amy.replay(bob.tape());
/// bob.replay(amy.tape());
/// // once we call .tape() once, it will no longer be available
/// assert_eq!(bob.tape().len(), 0)
/// assert_eq!(amy.tape().len(), 0)
/// ```
#[derive(Deserialize)]
pub struct SyncedList<T: Clone> {
    list: List<T, usize>,
    actor: usize,
    #[serde(skip)] 
    tape: Vec<Op<T, usize>>,
    
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

impl<'a, T: Clone + Debug> Debug for SyncedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncedList")
            .field("list", &self.list.read::<Vec<_>>())
            .field("actor", &self.actor)
            .finish()
    }
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

impl<T: Clone> SyncedList<T> {
    pub fn new() -> Self {
        SyncedList { list: List::new(), actor: 0, tape: vec![] } 
    }

    /// Get the length of the list.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Grab the clone of an element from the list
    pub fn index(&self, idx: usize) -> T {
        if self.len() > idx {
            self.list.position(idx).unwrap().clone()
        } else {
            panic!("index out of bounds: length is {} but index is {}",
                   self.len(), idx);
        }
    }

    /// Get an element from the list, optionally setting it.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut list:SyncedList<u32> = SyncedList::new():
    /// list.push(1);
    /// assert_eq!(*list.lock(0).unwrap(), 1);
    /// *list.lock(0).unwrap() = 2;
    /// assert_eq!(*list.lock(0).unwrap(), 2);
    /// ```
    pub fn lock(&mut self, idx: usize) -> Option<SyncedListGuard<T>> {
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
}


impl<T: Clone+Sync> Taped<usize> for SyncedList<T> {
    type Operation =  Op<T, usize>;

    /// Synchronize your list against a tape
    fn replay(&mut self, tape: Vec<Op<T, usize>>) {
        tape.into_iter().for_each(|x| self.list.apply(x));
    }

    /// Grab the tape of the list, removing its tape.
    ///
    /// # Note 
    /// If the tape is not published onto the wire, it  will be lost forever
    /// and not recoverable. 
    fn tape(&mut self) -> Vec<Op<T, usize>> {
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
