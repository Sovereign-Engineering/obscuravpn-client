use std::time::Duration;

use crate::backoff::Backoff;

#[test]
fn test() {
    for _ in 0..100 {
        let delays = Backoff::BACKGROUND.take(10).collect::<Vec<_>>();
        assert_eq!(delays[0], Duration::ZERO);
        assert!(delays[1] > Duration::ZERO);
        assert!(delays[1] <= Duration::from_secs(1));
        assert!(delays[2] > Duration::from_secs(1));
        assert!(delays[2] <= Duration::from_secs(2));
        assert!(delays[3] > Duration::from_secs(2));
        assert!(delays[3] <= Duration::from_secs(4));
        assert!(delays[4] > Duration::from_secs(4));
        assert!(delays[4] <= Duration::from_secs(8));
        assert!(delays[5] > Duration::from_secs(8));
        assert!(delays[5] <= Duration::from_secs(16));
        assert!(delays[6] > Duration::from_secs(16));
        assert!(delays[6] <= Duration::from_secs(32));
        assert!(delays[7] > Duration::from_secs(30));
        assert!(delays[7] <= Duration::from_secs(60));
        assert!(delays[8] > Duration::from_secs(30));
        assert!(delays[8] <= Duration::from_secs(60));
        assert!(delays[9] > Duration::from_secs(30));
        assert!(delays[9] <= Duration::from_secs(60));
    }
}
