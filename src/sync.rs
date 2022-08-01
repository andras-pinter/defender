use parking_lot::RwLock;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::error::GuardError;
use crate::state::State;
use crate::{GuardConfig, Timeout};

pub struct SyncGuard<T: Clone> {
    value: Arc<RwLock<State<T>>>,
    config: GuardConfig,
}

impl<T: Clone> Clone for SyncGuard<T> {
    fn clone(&self) -> Self {
        SyncGuard {
            value: self.value.clone(),
            config: self.config.clone(),
        }
    }
}

impl<T: Clone> Default for SyncGuard<T> {
    fn default() -> Self {
        SyncGuard {
            value: Arc::new(RwLock::default()),
            config: GuardConfig::default(),
        }
    }
}

impl<T: Clone> SyncGuard<T> {
    pub fn new(config: GuardConfig) -> Self {
        SyncGuard {
            config,
            ..Default::default()
        }
    }

    pub fn wait(&self) -> Result<Arc<T>, GuardError> {
        match self.config.timeout {
            Timeout::Instant => match self.value.read().deref() {
                State::Value(val) => Ok(val.clone()),
                State::UnSet => Err(GuardError::Timeout),
                State::Killed => Err(GuardError::Killed),
            },
            Timeout::Infinite => loop {
                match self.value.read().deref() {
                    State::Value(val) => break Ok(val.clone()),
                    State::UnSet => continue,
                    State::Killed => break Err(GuardError::Killed),
                }
            },
            Timeout::Duration(timeout) => {
                let t0 = std::time::Instant::now();
                while t0.elapsed() <= timeout {
                    match self.value.read().deref() {
                        State::Value(val) => return Ok(val.clone()),
                        State::UnSet => continue,
                        State::Killed => return Err(GuardError::Killed),
                    }
                }

                Err(GuardError::Timeout)
            }
        }
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

        match state.deref() {
            State::UnSet | State::Killed => Ok(None),
            State::Value(val) => {
                let value = (**val).to_owned();
                *state = State::UnSet;

                Ok(Some(value))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::GuardError;
    use crate::sync::SyncGuard;
    use crate::{GuardConfig, Timeout};
    use std::time::Duration;

    const EPSILON_MILLIS: u128 = 10;

    #[test]
    fn test_wait_for_value() {
        let guard = SyncGuard::default();
        let mut t_guard = guard.clone();
        let _t = std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            std::thread::sleep(Duration::from_millis(100));
            assert!(t_guard.set(t0).is_ok());
        });

        let value = guard.wait();
        assert!(value.is_ok());
        assert!(value.unwrap().elapsed().as_millis() - 100 <= EPSILON_MILLIS);
    }

    #[test]
    fn test_wait_for_value_with_timeout() {
        let config = GuardConfig {
            timeout: Timeout::Duration(Duration::from_millis(120)),
        };
        let guard = SyncGuard::new(config);
        let mut t_guard = guard.clone();
        let _t = std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert!(t_guard.set(t0).is_ok());
        });

        let value = guard.wait();
        assert!(value.is_ok());
        assert!(value.unwrap().elapsed().as_millis() - 100 <= EPSILON_MILLIS);
    }

    #[test]
    fn test_wait_for_value_timed_out() {
        let timeout = Duration::from_millis(50);
        let config = GuardConfig {
            timeout: Timeout::Duration(timeout),
        };
        let guard = SyncGuard::new(config);
        let mut t_guard = guard.clone();
        let _t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            assert!(t_guard.set(42u8).is_ok());
        });

        let t0 = std::time::Instant::now();
        let value = guard.wait();
        assert!(t0.elapsed().as_millis() - timeout.as_millis() <= EPSILON_MILLIS);
        assert!(value.is_err());
    }

    #[test]
    fn test_wait_for_value_with_multiple_timout() {
        let config = GuardConfig {
            timeout: Timeout::Duration(Duration::from_millis(60)),
        };
        let guard = SyncGuard::new(config);
        let mut t_guard = guard.clone();
        let _t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            assert!(t_guard.set(42u8).is_ok());
        });

        assert!(guard.wait().is_err());
        assert!(guard.wait().is_ok());
    }

    #[test]
    fn test_wait_killed() {
        let mut guard = SyncGuard::<u8>::default();
        let t_guard = guard.clone();
        let t = std::thread::spawn(move || {
            let result = t_guard.wait();
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), GuardError::Killed);
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(guard.kill().is_ok());
        t.join().expect("failed to wait guard thread");
    }

    #[test]
    fn test_value_set_after_kill() {
        let mut guard = SyncGuard::default();
        let mut t_guard = guard.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let result = t_guard.set(42u8);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), GuardError::Killed);
        });

        std::thread::sleep(Duration::from_millis(50));
        assert!(guard.kill().is_ok());
        t.join().expect("failed to wait guard thread")
    }

    #[test]
    fn test_resetting_a_value() {
        let config = GuardConfig {
            timeout: Timeout::Duration(Duration::from_millis(100)),
        };
        let mut guard = SyncGuard::<u8>::new(config);
        assert!(guard.set(42).is_ok());

        assert!(guard.wait().is_ok());
        assert_eq!(*guard.wait().unwrap(), 42);

        let res = guard.reset();
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), Some(42));

        assert!(guard.wait().is_err());
    }

    #[test]
    fn test_killed_an_elapsed_guard() {
        let mut guard = SyncGuard::<u8>::default();
        assert!(guard.set(42).is_ok());
        assert!(guard.kill().is_err());
        assert_eq!(guard.kill().unwrap_err(), GuardError::UnableToKilled);
    }

    #[test]
    fn test_instant_retrieving_value() {
        let config = GuardConfig {
            timeout: Timeout::Instant,
        };
        let mut guard = SyncGuard::<u8>::new(config);
        assert!(guard.wait().is_err());
        assert!(guard.set(42).is_ok());
        assert!(guard.wait().is_ok());
        assert_eq!(*guard.wait().unwrap(), 42);
    }
}
