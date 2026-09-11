use std::time::{Duration, Instant};

/// Optional debounce; continuously changing OCR must still reach the translator.
#[derive(Default)]
pub(super) struct Stability {
    candidate: String,
    changed_at: Option<Instant>,
    pending_since: Option<Instant>,
}

impl Stability {
    pub(super) fn ready(&mut self, text: &str, now: Instant, delay_ms: u64, once: bool) -> bool {
        if text.is_empty() {
            self.reset();
            return false;
        }
        let pending = *self.pending_since.get_or_insert(now);
        if self.candidate != text {
            self.candidate = text.to_owned();
            self.changed_at = Some(now);
        }
        once || delay_ms == 0
            || now.duration_since(self.changed_at.unwrap_or(now)) >= Duration::from_millis(delay_ms)
            || now.duration_since(pending) >= Duration::from_millis(delay_ms.saturating_mul(2).max(1000))
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mort_default_and_snapshot_translate_first_reading() {
        let now = Instant::now();
        assert!(Stability::default().ready("Hello", now, 0, false));
        assert!(Stability::default().ready("Hello", now, 2000, true));
    }

    #[test]
    fn optional_wait_handles_stable_and_continuously_changing_text() {
        let now = Instant::now();
        let mut gate = Stability::default();
        assert!(!gate.ready("H", now, 500, false));
        assert!(!gate.ready("He", now + Duration::from_millis(400), 500, false));
        assert!(gate.ready("He", now + Duration::from_millis(900), 500, false));
        gate.reset();
        for step in 0..5 {
            assert!(!gate.ready(&step.to_string(), now + Duration::from_millis(step * 200), 500, false));
        }
        assert!(gate.ready("different OCR again", now + Duration::from_millis(1000), 500, false));
    }

    #[test]
    fn empty_text_and_completed_translation_reset_deadline() {
        let now = Instant::now();
        let mut gate = Stability::default();
        assert!(!gate.ready("A", now, 500, false));
        assert!(!gate.ready("", now + Duration::from_secs(1), 500, false));
        assert!(!gate.ready("B", now + Duration::from_secs(2), 500, false));
        gate.reset();
        assert!(!gate.ready("C", now + Duration::from_secs(3), 500, false));
    }
}
