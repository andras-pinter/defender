use parking_lot::{Mutex, RwLock};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::error::GuardError;
use crate::state::State;
use crate::{GuardConfig, Timeout};

pub struct AsyncGuard<T: Clone> {
    value: Arc<RwLock<State<T>>>,
    config: GuardConfig,

    t0: Arc<Mutex<Option<std::time::Instant>>>,
}

impl<T: Clone> Clone for AsyncGuard<T> {
    fn clone(&self) -> Self {
        AsyncGuard {
            value: self.value.clone(),
            config: self.config.clone(),
            t0: self.t0.clone(),
        }
    }
}

impl<T: Clone> Default for AsyncGuard<T> {
    fn default() -> Self {
        AsyncGuard {
            value: Arc::new(RwLock::default()),
            config: GuardConfig::default(),
            t0: Arc::new(Mutex::default()),
        }
    }
}

impl<T: Clone> AsyncGuard<T> {
    pub fn new(config: GuardConfig) -> Self {
        AsyncGuard {
            config,
            ..Default::default()
        }
    }
}

impl<T: Clone> AsyncGuard<T> {
    pub async fn wait(&self) -> Result<Arc<T>, GuardError> {
        self.await
    }

    pub fn set(&mut self, value: T) -> Result<(), GuardError> {
        match self.value.write().deref_mut() {
            State::Killed => Err(GuardError::Killed),
            state => {
                *state = State::Value(Arc::new(value));
                Ok(())
            }
        }
    }

    pub fn kill(&mut self) -> Result<(), GuardError> {
        match self.value.write().deref_mut() {
            State::Value(_) => Err(GuardError::UnableToKilled),
            state => {
                *state = State::Killed;
                Ok(())
            }
        }
    }

    pub fn reset(&mut self) -> Result<Option<T>, GuardError> {
        let mut state = self.value.write();

        match state.deref_mut() {
            State::UnSet | State::Killed => Ok(None),
            State::Value(val) => {
                let value = (**val).to_owned();
                *state = State::UnSet;

                Ok(Some(value))
            }
        }
    }
}

impl<T: Clone> Future for &AsyncGuard<T> {
    type Output = Result<Arc<T>, GuardError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.config.timeout {
            Timeout::Instant => match self.value.read().deref() {
                State::Value(val) => Poll::Ready(Ok(val.clone())),
                State::UnSet => Poll::Ready(Err(GuardError::Timeout)),
                State::Killed => Poll::Ready(Err(GuardError::Killed)),
            },
            Timeout::Infinite => match self.value.read().deref() {
                State::Value(val) => Poll::Ready(Ok(val.clone())),
                State::Killed => Poll::Ready(Err(GuardError::Killed)),
                State::UnSet => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            },
            Timeout::Duration(timeout) => {
                if self.t0.lock().is_none() {
                    let t0 = std::time::Instant::now();
                    *self.t0.lock().deref_mut() = Some(t0);
                }

                match self.t0.lock().deref() {
                    Some(t0) if t0.elapsed() <= timeout => match self.value.read().deref() {
                        State::Value(val) => Poll::Ready(Ok(val.clone())),
                        State::Killed => Poll::Ready(Err(GuardError::Killed)),
                        State::UnSet => Poll::Pending,
                    },
                    Some(_) => Poll::Ready(Err(GuardError::Timeout)),
                    None => Poll::Pending,
                }
            }
        }
    }
}
