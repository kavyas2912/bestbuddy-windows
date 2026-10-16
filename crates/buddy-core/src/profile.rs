use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Consents {
    pub profile_storage: bool,
    pub contacts_storage: bool,
    pub microphone: bool,
    pub voice_processing: bool,
    pub personalization: bool,
    pub local_learning: bool,
    pub live_information: bool,
    pub external_services: bool,
    pub emergency_contact: bool,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub relationship: String,
    pub phone: String,
    pub email: String,
    pub signal_link: String,
    pub messenger_link: String,
    pub slack_link: String,
    pub discord_link: String,
    pub zoom_link: String,
    pub teams_link: String,
    pub channel: String,
    pub birthday: String,
    pub anniversary: String,
    pub emergency_priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub date: String,
    pub time: String,
    pub annual: bool,
    pub lead_days: u32,
    pub enabled: bool,
    pub contact_id: String,
    pub channel: String,
}

impl Reminder {
    pub fn due_on(&self, today: NaiveDate) -> bool {
        if !self.enabled {
            return false;
        }
        let Ok(date) = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") else {
            return false;
        };
        let Some(target) = today.checked_add_signed(chrono::Duration::days(self.lead_days as i64))
        else {
            return false;
        };
        if target < date || self.lead_days > 366 {
            return false;
        }
        if self.annual {
            // A leap-day anniversary is observed on February 28 in other years.
            let occurrence = NaiveDate::from_ymd_opt(target.year(), date.month(), date.day())
                .or_else(|| NaiveDate::from_ymd_opt(target.year(), 2, 28));
            occurrence == Some(target)
        } else {
            date == target
        }
    }
}

pub fn age_on(birthdate: &str, today: NaiveDate) -> Option<i32> {
    let born = NaiveDate::parse_from_str(birthdate, "%Y-%m-%d").ok()?;
    if born > today {
        return None;
    }
    Some(
        today.year()
            - born.year()
            - i32::from((today.month(), today.day()) < (born.month(), born.day())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn age_changes_on_birthday_and_rejects_future() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        assert_eq!(age_on("1954-09-23", day), Some(71));
        assert_eq!(age_on("1954-09-22", day), Some(72));
        assert_eq!(age_on("2027-01-01", day), None);
    }
    #[test]
    fn reminder_rejects_occurrences_before_start_and_excessive_lead() {
        let mut r = Reminder {
            date: "2028-01-01".into(),
            annual: true,
            lead_days: 2,
            enabled: true,
            ..Default::default()
        };
        assert!(!r.due_on(NaiveDate::from_ymd_opt(2026, 12, 30).unwrap()));
        assert!(r.due_on(NaiveDate::from_ymd_opt(2027, 12, 30).unwrap()));
        r.lead_days = 367;
        assert!(!r.due_on(NaiveDate::from_ymd_opt(2026, 12, 30).unwrap()));
    }
    #[test]
    fn reminder_crosses_year_and_handles_leap_day() {
        let mut r = Reminder {
            date: "2000-01-01".into(),
            annual: true,
            lead_days: 2,
            enabled: true,
            ..Default::default()
        };
        assert!(r.due_on(NaiveDate::from_ymd_opt(2026, 12, 30).unwrap()));
        r.date = "2000-02-29".into();
        r.lead_days = 0;
        assert!(r.due_on(NaiveDate::from_ymd_opt(2027, 2, 28).unwrap()));
        assert!(!r.due_on(NaiveDate::from_ymd_opt(2028, 2, 28).unwrap()));
        r.enabled = false;
        assert!(!r.due_on(NaiveDate::from_ymd_opt(2028, 2, 29).unwrap()));
    }
}
