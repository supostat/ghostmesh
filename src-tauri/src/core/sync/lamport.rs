#[derive(Debug, Clone)]
pub struct LamportClock {
    counter: u64,
}

impl LamportClock {
    pub fn new() -> Self {
        LamportClock { counter: 0 }
    }

    pub fn with_value(value: u64) -> Self {
        LamportClock { counter: value }
    }

    pub fn current(&self) -> u64 {
        self.counter
    }

    pub fn tick(&mut self) -> u64 {
        self.counter = self.counter.saturating_add(1);
        self.counter
    }

    pub fn on_send(&mut self) -> u64 {
        self.tick()
    }

    pub fn on_receive(&mut self, remote_ts: u64) -> u64 {
        self.counter = self.counter.max(remote_ts).saturating_add(1);
        self.counter
    }

    pub fn merge(&mut self, other_ts: u64) {
        self.counter = self.counter.max(other_ts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let clock = LamportClock::new();
        assert_eq!(clock.current(), 0);
    }

    #[test]
    fn with_value_starts_at_given_value() {
        let clock = LamportClock::with_value(42);
        assert_eq!(clock.current(), 42);
    }

    #[test]
    fn tick_increments_by_one() {
        let mut clock = LamportClock::new();
        assert_eq!(clock.tick(), 1);
        assert_eq!(clock.tick(), 2);
        assert_eq!(clock.current(), 2);
    }

    #[test]
    fn on_send_increments_like_tick() {
        let mut clock = LamportClock::with_value(5);
        assert_eq!(clock.on_send(), 6);
        assert_eq!(clock.current(), 6);
    }

    #[test]
    fn on_receive_with_higher_remote() {
        let mut clock = LamportClock::with_value(3);
        let result = clock.on_receive(10);
        assert_eq!(result, 11);
        assert_eq!(clock.current(), 11);
    }

    #[test]
    fn on_receive_with_lower_remote() {
        let mut clock = LamportClock::with_value(10);
        let result = clock.on_receive(3);
        assert_eq!(result, 11);
        assert_eq!(clock.current(), 11);
    }

    #[test]
    fn on_receive_with_equal_remote() {
        let mut clock = LamportClock::with_value(5);
        let result = clock.on_receive(5);
        assert_eq!(result, 6);
        assert_eq!(clock.current(), 6);
    }

    #[test]
    fn merge_takes_max_without_increment() {
        let mut clock = LamportClock::with_value(3);
        clock.merge(10);
        assert_eq!(clock.current(), 10);
    }

    #[test]
    fn merge_keeps_local_when_higher() {
        let mut clock = LamportClock::with_value(10);
        clock.merge(3);
        assert_eq!(clock.current(), 10);
    }

    #[test]
    fn merge_with_equal_keeps_value() {
        let mut clock = LamportClock::with_value(7);
        clock.merge(7);
        assert_eq!(clock.current(), 7);
    }

    #[test]
    fn tick_saturates_at_u64_max() {
        let mut clock = LamportClock::with_value(u64::MAX);
        let result = clock.tick();
        assert_eq!(result, u64::MAX);
    }

    #[test]
    fn on_receive_saturates_at_u64_max() {
        let mut clock = LamportClock::with_value(0);
        let result = clock.on_receive(u64::MAX);
        assert_eq!(result, u64::MAX);
    }

    #[test]
    fn sequential_operations() {
        let mut clock = LamportClock::new();
        clock.tick(); // 1
        clock.on_receive(5); // max(1, 5) + 1 = 6
        clock.merge(3); // max(6, 3) = 6
        clock.on_send(); // 7
        assert_eq!(clock.current(), 7);
    }
}
