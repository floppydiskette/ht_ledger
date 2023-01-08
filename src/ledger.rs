use std::collections::BTreeMap;
use std::num::Wrapping;
use std::ops::{Add, Sub};
use ht_cal::datetime::{HDateTime, Month, MonthStatus};
use ht_timeparser::HTDate;
use serde::{Serialize, Deserialize};

pub const OUT_PATH: &str = "/opt/ht_ledger";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Hash)]
pub struct HashDay {
    pub day_lo: Wrapping<u128>, // count of days since the start of huskitopian time, update this when we run out of days
    pub day_hi: Wrapping<u128>, // count of days since the start of huskitopian time, update this when we run out of days
}

impl Sub for HashDay {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let mut day_lo = self.day_lo;
        let mut day_hi = self.day_hi;
        if rhs.day_lo > day_lo {
            day_lo = day_lo + (Wrapping(u128::MAX) - rhs.day_lo);
            day_hi -= 1;
        }
        day_lo -= rhs.day_lo;
        day_hi -= rhs.day_hi;
        Self {
            day_lo,
            day_hi,
        }
    }
}

impl Add for HashDay {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut day_lo = self.day_lo;
        let mut day_hi = self.day_hi;
        day_lo += rhs.day_lo;
        if day_lo < self.day_lo {
            day_hi += 1;
        }
        day_hi += rhs.day_hi;
        Self {
            day_lo,
            day_hi,
        }
    }
}

fn u8_to_month(i: u8) -> (MonthStatus, Month) {
    match i {
        0 => (MonthStatus::Greater, Month::Zero),
        1 => (MonthStatus::Lesser, Month::Zero),
        2 => (MonthStatus::Greater, Month::Niktvirin),
        3 => (MonthStatus::Lesser, Month::Niktvirin),
        4 => (MonthStatus::Greater, Month::Apress),
        5 => (MonthStatus::Lesser, Month::Apress),
        6 => (MonthStatus::Greater, Month::Smosh),
        7 => (MonthStatus::Lesser, Month::Smosh),
        8 => (MonthStatus::Greater, Month::Funny),
        9 => (MonthStatus::Lesser, Month::Funny),
        _ => panic!("invalid month"),
    }
}
fn month_to_u8(month: (MonthStatus, Month)) -> u8 {
    match month {
        (MonthStatus::Greater, Month::Zero) => 0,
        (MonthStatus::Lesser, Month::Zero) => 1,
        (MonthStatus::Greater, Month::Niktvirin) => 2,
        (MonthStatus::Lesser, Month::Niktvirin) => 3,
        (MonthStatus::Greater, Month::Apress) => 4,
        (MonthStatus::Lesser, Month::Apress) => 5,
        (MonthStatus::Greater, Month::Smosh) => 6,
        (MonthStatus::Lesser, Month::Smosh) => 7,
        (MonthStatus::Greater, Month::Funny) => 8,
        (MonthStatus::Lesser, Month::Funny) => 9,
    }
}

impl HashDay { // fixme: theoretically we should be going off of both day_lo and day_hi, but rust's support for 256 bit integers is still in the works
    pub fn from_hdatetime(hdt: &HDateTime) -> Self {
        let mut days = 0;
        let mut years = hdt.year;
        days += years * 10 * 24; // 10 months per year, 24 days per month
        let mut months = month_to_u8(hdt.month); // 0 is the first month, 1 is the second month, etc.
        while months > 0 {
            days += 24;
            months -= 1;
        }
        days += hdt.day as u128;
        Self {
            day_lo: Wrapping(days),
            day_hi: Wrapping(0),
        }
    }

    pub fn to_hdatetime(&self) -> HDateTime {
        let mut days = self.day_lo.0;
        let mut years = 0;
        while days >= 10 * 24 {
            days -= 10 * 24;
            years += 1;
        }
        let mut months = 0;
        while days >= 24 {
            days -= 24;
            months += 1;
        }
        let mut hdt = HDateTime::new();
        hdt.year = years;
        hdt.month = u8_to_month(months);
        hdt.day = days as u8;
        hdt
    }

    pub fn from_packetdata(packetdata: &ht_cal::packet::PacketData) -> Self {
        let mut hdt = HDateTime::new();
        hdt.year = packetdata.year;
        hdt.month = packetdata.month;
        hdt.day = packetdata.day;
        Self::from_hdatetime(&hdt)
    }
}

#[derive(Serialize, Deserialize)]
pub struct HLedger {
    pub day_seconds: BTreeMap<HashDay, u128>,
}

#[derive(Serialize, Deserialize)]
pub struct HLedgerRecord {
    pub day: String,
    pub seconds: u128,
}

impl HLedger {
    pub fn load() -> Self {
        let data = std::fs::read(format!("{}/ledger.bin", OUT_PATH));
        if let Err(e) = data {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Self {
                    day_seconds: BTreeMap::new(),
                };
            }
            panic!("error loading ledger: {}", e);
        }
        let data = data.unwrap();
        let ledger: Self = rmp_serde::from_slice(&data).unwrap();
        ledger
    }

    pub fn save(self) {
        // copy ledger to ledger.bak
        let attempt = std::fs::copy(format!("{}/ledger.bin", OUT_PATH), format!("{}/ledger.bak", OUT_PATH));
        // if we fail to copy, check to see if a ledger.bin exists; if it does, panic; if it doesn't, we're good
        if let Err(e) = attempt {
            if e.kind() == std::io::ErrorKind::NotFound {
                // we're good
            } else {
                panic!("error backing up ledger: {}", e);
            }
        }
        let data = rmp_serde::to_vec(&self).unwrap();
        std::fs::write(format!("{}/ledger.bin", OUT_PATH), data).unwrap();
    }

    pub fn import_from_htcal(&mut self, history: &ht_cal::history::HistoryData, today: &ht_cal::packet::PacketData) {
        // current day in hashday format
        let mut current_day = HashDay::from_packetdata(today).sub(HashDay{day_lo: Wrapping(1), day_hi: Wrapping(0)});
        // the current day in the history should be [0], so align properly with that
        for day in history.last_ten_seconds_per_day.iter() {
            if day != &0 {
                self.day_seconds.insert(current_day, *day);
                current_day = current_day.sub(HashDay {
                    day_lo: Wrapping(1),
                    day_hi: Wrapping(0),
                });
            }
        }
    }

    pub fn collect(&self, day: &HTDate) -> Vec<HLedgerRecord> {
        let mut day = HashDay::from_hdatetime(&day.to_hdatetime());
        let mut records = vec![];
        // get day and 24 previous days
        for _ in 0..24 {
            if let Some(seconds) = self.day_seconds.get(&day) {
                records.push(HLedgerRecord {
                        day: HTDate::from_hdatetime(&day.to_hdatetime()).to_string(),
                        seconds: *seconds,
                    });
            }
            day = day.sub(HashDay {
                day_lo: Wrapping(1),
                day_hi: Wrapping(0),
            });
        }
        records
    }
}