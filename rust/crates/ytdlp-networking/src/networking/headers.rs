/// Canonicalize a header name the same way Python's `str.title()` does for
/// the ASCII header names used by yt-dlp.
fn canonical_header_name(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Case-insensitive headers that retain the spelling supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    values: IndexMap<String, String>,
    original_names: IndexMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {
        let name = name.as_ref();
        let canonical = canonical_header_name(name);
        self.original_names
            .insert(canonical.clone(), name.to_owned());
        self.values
            .insert(canonical, value.as_ref().trim().to_owned());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .get(&canonical_header_name(name))
            .map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(&canonical_header_name(name))
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        let canonical = canonical_header_name(name);
        self.original_names.shift_remove(&canonical);
        self.values.shift_remove(&canonical)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Return the case-sensitive view used when adapting to another client.
    pub fn sensitive(&self) -> IndexMap<String, String> {
        self.values
            .iter()
            .map(|(canonical, value)| {
                (
                    self.original_names
                        .get(canonical)
                        .cloned()
                        .unwrap_or_else(|| canonical.clone()),
                    value.clone(),
                )
            })
            .collect()
    }
}

/// Case-insensitive response headers that retain repeated field values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseHeaders {
    values: IndexMap<String, Vec<String>>,
    original_names: IndexMap<String, String>,
}

impl ResponseHeaders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {
        let name = name.as_ref();
        let canonical = canonical_header_name(name);
        self.original_names
            .entry(canonical.clone())
            .or_insert_with(|| name.to_owned());
        self.values
            .entry(canonical)
            .or_default()
            .push(value.as_ref().trim().to_owned());
    }

    pub fn get_all(&self, name: &str) -> Vec<&str> {
        self.values
            .get(&canonical_header_name(name))
            .map(|values| values.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.get_all(name).into_iter().next()
    }

    /// Match yt-dlp's `Response.get_header` behavior.
    pub fn get_header(&self, name: &str) -> Option<String> {
        let values = self.get_all(name);
        if values.is_empty() {
            return None;
        }
        if canonical_header_name(name) == "Set-Cookie" {
            Some(values[0].to_owned())
        } else {
            Some(values.join(", "))
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(&canonical_header_name(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().flat_map(|(canonical, values)| {
            let name = self
                .original_names
                .get(canonical)
                .map(String::as_str)
                .unwrap_or(canonical.as_str());
            values.iter().map(move |value| (name, value.as_str()))
        })
    }
}
