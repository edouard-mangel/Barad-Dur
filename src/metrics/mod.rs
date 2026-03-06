pub mod evolution;
pub mod health;
pub mod hygiene;
pub mod team;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MetricValue {
    pub name: String,
    pub description: String,
    pub raw_value: RawValue,
    pub score: u32, // 0-100
}

#[derive(Debug, Clone, Serialize)]
pub enum RawValue {
    Integer(i64),
    Float(f64),
    Percentage(f64),
    Count(usize),
    Text(String),
    List(Vec<String>),
}

impl std::fmt::Display for RawValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawValue::Integer(v) => write!(f, "{}", v),
            RawValue::Float(v) => write!(f, "{:.2}", v),
            RawValue::Percentage(v) => write!(f, "{:.0}%", v),
            RawValue::Count(v) => write!(f, "{}", v),
            RawValue::Text(v) => write!(f, "{}", v),
            RawValue::List(v) => write!(f, "{}", v.join(", ")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryResult {
    pub name: String,
    pub score: u32,
    pub metrics: Vec<MetricValue>,
}

impl CategoryResult {
    /// Compute category score as average of metric scores.
    pub fn compute_score(mut self) -> Self {
        if self.metrics.is_empty() {
            self.score = 0;
        } else {
            let total: u32 = self.metrics.iter().map(|m| m.score).sum();
            self.score = total / self.metrics.len() as u32;
        }
        self
    }
}
