use std::cmp::Ordering;

use time::{OffsetDateTime, PrimitiveDateTime};

use crate::{configs::event::CONFIG, db::models::User};

use super::user::is_admin;

pub fn primitive_now() -> PrimitiveDateTime {
    let utc_now = OffsetDateTime::now_utc();
    let converted = utc_now.to_offset(CONFIG.timezone);

    PrimitiveDateTime::new(converted.date(), converted.time())
}

pub fn cmp_period(time: OffsetDateTime) -> Ordering {
    if let Some(start) = CONFIG.start_at {
        let start = start.assume_offset(CONFIG.timezone);
        if time < start {
            return Ordering::Less;
        }
    }

    if let Some(end) = CONFIG.end_at {
        let end = end.assume_offset(CONFIG.timezone);
        if time > end {
            return Ordering::Greater;
        }
    }

    Ordering::Equal
}

pub fn is_available(user: Option<&User>) -> bool {
    if let Some(user) = user {
        if is_admin(user) {
            return true;
        }
    }

    if cmp_period(OffsetDateTime::now_utc()) == Ordering::Less {
        return false;
    }

    true
}
