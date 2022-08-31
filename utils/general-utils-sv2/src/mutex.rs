use std::sync::{Mutex as Mutex_, MutexGuard, PoisonError};

#[derive(Debug)]
pub struct Mutex<T: ?Sized>(Mutex_<T>);

/// Safer Mutex:
///
/// This Mutex implement safe_lock and not lock.
/// safe_lock take closure as argument.
/// The closure will handle the value protected by the Mutex. Safe_lock return whatever the
/// closure return.
/// The closure API is superior to the canonical API cause it enfoce the drop
/// of the MutexGuard lowering the risk of deadlock.
/// The closure API is inferior to the canonical API cause can not be used
/// everywhere.
impl<T> Mutex<T> {
    /// Take a cluser that handle the value protected by the Mutex, it returns Result that
    /// contains whatever the closure returns. If the Mutex is poisoned will return an error.
    /// Unwrap the returned value is the right thing to do when handling of a poisoned Mutex is not
    /// possible.
    pub fn safe_lock<F, Ret>(&self, thunk: F) -> Result<Ret, PoisonError<MutexGuard<'_, T>>>
    where
        F: FnOnce(&mut T) -> Ret,
    {
        let mut lock = self.0.lock()?;
        let return_value = thunk(&mut *lock);
        drop(lock);
        Ok(return_value)
    }

    pub fn new(v: T) -> Self {
        Mutex(Mutex_::new(v))
    }

    pub fn to_remove(&self) -> Result<MutexGuard<'_, T>, PoisonError<MutexGuard<'_, T>>> {
        self.0.lock()
    }
}
