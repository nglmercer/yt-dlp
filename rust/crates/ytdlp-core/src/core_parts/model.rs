/// Implementation state used by the migration matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Rust,
    Todo,
}

/// A capability entry used by the migration matrix and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub name: &'static str,
    pub mode: EngineMode,
}

/// A JSON-compatible, insertion-order-preserving info dictionary.
///
/// This is intentionally a transitional representation. Non-JSON Python
/// values and lazy entries will need explicit internal variants before the
/// Rust engine can claim full embedding/API parity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InfoDict(IndexMap<String, Value>);

impl InfoDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        self.0.insert(key.into(), value)
    }

    pub fn insert_if_some<T>(&mut self, key: impl Into<String>, value: Option<T>)
    where
        T: Serialize,
    {
        if let Some(value) = value {
            self.insert(key, serde_json::to_value(value).unwrap_or(Value::Null));
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.0.shift_remove(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        })
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn as_map(&self) -> &IndexMap<String, Value> {
        &self.0
    }

    pub fn into_map(self) -> IndexMap<String, Value> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreErrorKind {
    InvalidInput,
    Unsupported,
    MissingField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    pub kind: CoreErrorKind,
    pub message: String,
}

impl CoreError {
    pub fn new(kind: CoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CoreError {}
