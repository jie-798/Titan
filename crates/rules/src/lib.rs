pub mod domain;
pub mod engine;
pub mod geoip;
pub mod geosite;
pub mod ipcidr;

pub use engine::{MatchRequest, MatchResult, RuleEngine};

#[cfg(test)]
mod tests;
