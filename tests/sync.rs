use defender::error::GuardError;
use defender::sync::SyncGuard;
use defender::{GuardConfig, Timeout};

const EPSILON_MILLIS: u128 = 10;
const TEST_CONFIG: GuardConfig = GuardConfig {
    timeout: Timeout::Duration(std::time::Duration::from_millis(100)),
};

#[test]
fn test_sync_guard() {
    let guard = SyncGuard::<String>::new(TEST_CONFIG);
    let mut guard_clone = guard.clone();

    let guard_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        let message = String::from("Hello SyncGuard");
        assert!(guard_clone.set(message).is_ok());
    });

    let result = guard.wait();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_ref(), "Hello SyncGuard");
    guard_thread.join().expect("Error while joining threads");
}

#[test]
fn test_sync_guard_timeout() {
    let guard = SyncGuard::<String>::new(TEST_CONFIG);
    assert!(guard.wait().is_err());
    assert!(matches!(guard.wait().unwrap_err(), GuardError::Timeout));
}

#[tokio::test]
async fn test_sync_guard_in_async_context() {
    let mut sleep_timeout = false;
    let mut guard_timeout = false;
    let guard = SyncGuard::<String>::new(TEST_CONFIG);
    let dur = tokio::time::Duration::from_millis(10);
    let sleep = tokio::time::sleep(dur);
    tokio::pin!(sleep);

    let t0 = std::time::Instant::now();
    tokio::select! {
        _ = &mut sleep => {
            sleep_timeout = true;
        }
        _ = async { guard.wait() } => {
            guard_timeout = true
        }
    };
    assert!(t0.elapsed().as_millis() - 100 < EPSILON_MILLIS);
    assert!(!sleep_timeout, "tokio sleep should not polled");
    assert!(guard_timeout, "guard should block async sleep");
}
