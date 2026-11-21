//! Business services. Each service works on a `rusqlite::Connection` so it can be tested with an
//! in-memory database; commands handle sessions and locking.

pub mod accounts;
pub mod adb;
pub mod audit;
pub mod checks;
pub mod dashboard;
pub mod devices;
pub mod frp;
pub mod orders;
pub mod pdf;
pub mod reports;
pub mod procedures;
pub mod reset;
pub mod settings;
pub mod transfer;
pub mod usb;
pub mod users;
