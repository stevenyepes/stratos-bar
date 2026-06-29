use crate::domain::action::Action;

pub const MS_PER_HOUR: u64 = 3_600_000;

pub const DEFAULT_HALF_LIFE_HOURS: f64 = 336.0;

pub fn current_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn decayed_score(
    frequency: u64,
    last_accessed_ms: u64,
    now_ms: u64,
    half_life_hours: f64,
) -> f64 {
    if last_accessed_ms > now_ms {
        return frequency as f64;
    }
    let age_ms = now_ms - last_accessed_ms;
    let age_hours = age_ms as f64 / MS_PER_HOUR as f64;
    let decay = (-age_hours / half_life_hours).exp();
    (frequency as f64) * decay
}

pub fn frecency_score(action: &Action, now_ms: u64, half_life_hours: f64) -> f64 {
    decayed_score(
        action.frequency,
        action.last_accessed,
        now_ms,
        half_life_hours,
    )
}

pub fn frecency_bonus(action: &Action, now_ms: u64) -> f64 {
    frecency_score(action, now_ms, DEFAULT_HALF_LIFE_HOURS)
}

pub fn compose_score(text_score: f32, action: Option<&Action>, now_ms: u64) -> f32 {
    let bonus = action
        .map(|a| frecency_bonus(a, now_ms) as f32)
        .unwrap_or(0.0);
    text_score + bonus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_action(id: &str, kind: &str, frequency: u64, last_accessed: u64) -> Action {
        Action {
            id: id.to_string(),
            kind: kind.to_string(),
            content: format!("/usr/bin/{id}"),
            name: id.to_string(),
            icon: None,
            last_accessed,
            frequency,
        }
    }

    #[test]
    fn decayed_score_at_zero_age_returns_full_frequency() {
        let now = 1_000_000_000_000;
        let score = decayed_score(10, now, now, 336.0);
        assert!((score - 10.0).abs() < 1e-6);
    }

    #[test]
    fn decayed_score_at_one_half_life_is_over_third() {
        let now = 1_000_000_000_000;
        let half_life_ms = (336.0 * MS_PER_HOUR as f64) as u64;
        let score = decayed_score(10, now - half_life_ms, now, 336.0);
        assert!((score - 10.0 / std::f64::consts::E).abs() < 1e-3);
    }

    #[test]
    fn decayed_score_at_one_day_is_mostly_intact() {
        let now = 1_000_000_000_000;
        let one_day_ms = 24 * MS_PER_HOUR;
        let score = decayed_score(10, now - one_day_ms, now, 336.0);
        assert!(score > 9.0, "score should be near full after 1 day, got {score}");
        assert!(score < 10.0, "score should have decayed at all, got {score}");
    }

    #[test]
    fn decayed_score_at_two_weeks_is_significantly_decayed() {
        let now = 1_000_000_000_000;
        let two_weeks_ms = 14 * 24 * MS_PER_HOUR;
        let score = decayed_score(10, now - two_weeks_ms, now, 336.0);
        assert!(score < 5.0, "score should be < half after 14d, got {score}");
        assert!(score > 0.0, "score must remain positive, got {score}");
    }

    #[test]
    fn decayed_score_future_timestamp_treated_as_zero_age() {
        let now = 1_000_000_000_000;
        let score = decayed_score(10, now + 1000, now, 336.0);
        assert!((score - 10.0).abs() < 1e-6);
    }

    #[test]
    fn decayed_score_with_zero_frequency_is_zero() {
        let now = 1_000_000_000_000;
        let score = decayed_score(0, now, now, 336.0);
        assert_eq!(score, 0.0);
        let score = decayed_score(0, now - 1000, now, 336.0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn decayed_score_custom_half_life_scales_correctly() {
        let now = 1_000_000_000_000;
        let one_hour_ms = MS_PER_HOUR;
        let score_1h = decayed_score(10, now - one_hour_ms, now, 1.0);
        let score_336h = decayed_score(10, now - one_hour_ms, now, 336.0);
        assert!(score_1h < score_336h);
        assert!((score_1h - 10.0 / std::f64::consts::E).abs() < 1e-6);
    }

    #[test]
    fn frecency_score_delegates_to_decayed_score() {
        let now = 1_000_000_000_000;
        let action = make_action("chrome", "app", 7, now);
        let direct = decayed_score(7, now, now, DEFAULT_HALF_LIFE_HOURS);
        let via_action = frecency_score(&action, now, DEFAULT_HALF_LIFE_HOURS);
        assert!((direct - via_action).abs() < 1e-9);
    }

    #[test]
    fn frecency_bonus_uses_default_half_life() {
        let now = 1_000_000_000_000;
        let action = make_action("chrome", "app", 7, now);
        let explicit = frecency_score(&action, now, DEFAULT_HALF_LIFE_HOURS);
        let bonus = frecency_bonus(&action, now);
        assert!((explicit - bonus).abs() < 1e-9);
    }

    #[test]
    fn compose_score_adds_bonus_when_action_present() {
        let now = 1_000_000_000_000;
        let action = make_action("chrome", "app", 10, now);
        let combined = compose_score(1.5, Some(&action), now);
        assert!((combined - (1.5 + 10.0)).abs() < 1e-6);
    }

    #[test]
    fn compose_score_returns_text_when_no_action() {
        let now = 1_000_000_000_000;
        let combined = compose_score(1.5, None, now);
        assert!((combined - 1.5).abs() < 1e-6);
    }

    #[test]
    fn compose_score_decays_with_age() {
        let now = 1_000_000_000_000;
        let one_day_ms = 24 * MS_PER_HOUR;
        let action_fresh = make_action("a", "app", 10, now);
        let action_old = make_action("b", "app", 10, now - one_day_ms);
        let fresh_combined = compose_score(0.0, Some(&action_fresh), now);
        let old_combined = compose_score(0.0, Some(&action_old), now);
        assert!(fresh_combined > old_combined);
        assert!(fresh_combined > 9.5);
        assert!(old_combined < fresh_combined);
    }

    #[test]
    fn more_frequent_beats_more_recent_at_same_age() {
        let now = 1_000_000_000_000;
        let one_day_ms = 24 * MS_PER_HOUR;
        let frequent = make_action("a", "app", 100, now - one_day_ms);
        let recent = make_action("b", "app", 1, now);
        let s_frequent = frecency_score(&frequent, now, DEFAULT_HALF_LIFE_HOURS);
        let s_recent = frecency_score(&recent, now, DEFAULT_HALF_LIFE_HOURS);
        assert!(s_frequent > s_recent);
    }

    #[test]
    fn current_time_ms_returns_monotonic_positive_value() {
        let t1 = current_time_ms();
        let t2 = current_time_ms();
        assert!(t2 >= t1);
        assert!(t1 > 0);
    }

    #[test]
    fn default_half_life_is_fourteen_days_in_hours() {
        assert!((DEFAULT_HALF_LIFE_HOURS - 336.0).abs() < 1e-9);
    }
}
