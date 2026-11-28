use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Clone, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

pub fn format_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339)
        .unwrap_or_else(|_| dt.unix_timestamp().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClock {
        time: OffsetDateTime,
    }

    impl Clock for MockClock {
        fn now(&self) -> OffsetDateTime {
            self.time
        }
    }

    #[test]
    fn test_system_clock() {
        let clock = SystemClock;
        let before = OffsetDateTime::now_utc();
        let now = clock.now();
        let after = OffsetDateTime::now_utc();
        assert!(before <= now);
        assert!(now <= after);
    }

    #[test]
    fn test_mock_clock() {
        let expected = OffsetDateTime::now_utc();
        let clock = MockClock { time: expected };
        assert_eq!(clock.now(), expected);
    }
}
