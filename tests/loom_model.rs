//! Reduced Loom model of the root-first reservation algorithm.

use std::sync::Arc;

use loom::sync::Mutex;
use loom::thread;

#[test]
fn root_locked_reservation_model_never_oversubscribes() {
    loom::model(|| {
        let state = Arc::new(Mutex::new((0_u64, 0_u64)));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let state = state.clone();
                thread::spawn(move || {
                    let mut state = state.lock().unwrap();
                    if state.0 + state.1 + 8 <= 10 {
                        state.1 += 8;
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let state = state.lock().unwrap();
        assert!(state.0 + state.1 <= 10);
    });
}

#[test]
fn reservation_release_racing_with_consume_preserves_the_limit() {
    loom::model(|| {
        let state = Arc::new(Mutex::new((0_u64, 8_u64)));
        let release_state = state.clone();
        let release = thread::spawn(move || {
            let mut state = release_state.lock().unwrap();
            state.1 -= 8;
        });
        let consume_state = state.clone();
        let consume = thread::spawn(move || {
            let mut state = consume_state.lock().unwrap();
            if state.0 + state.1 + 7 <= 10 {
                state.0 += 7;
            }
        });

        release.join().unwrap();
        consume.join().unwrap();
        let state = state.lock().unwrap();
        assert!(state.0 + state.1 <= 10);
        assert_eq!(state.1, 0);
    });
}

#[test]
fn reservation_commit_racing_with_consume_preserves_the_limit() {
    loom::model(|| {
        let state = Arc::new(Mutex::new((0_u64, 8_u64)));
        let commit_state = state.clone();
        let commit = thread::spawn(move || {
            let mut state = commit_state.lock().unwrap();
            state.1 -= 8;
            state.0 += 3;
        });
        let consume_state = state.clone();
        let consume = thread::spawn(move || {
            let mut state = consume_state.lock().unwrap();
            if state.0 + state.1 + 7 <= 10 {
                state.0 += 7;
            }
        });

        commit.join().unwrap();
        consume.join().unwrap();
        let state = state.lock().unwrap();
        assert!(state.0 + state.1 <= 10);
        assert_eq!(state.1, 0);
        assert!(state.0 == 3 || state.0 == 10);
    });
}
