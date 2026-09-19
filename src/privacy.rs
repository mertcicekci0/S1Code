//! Best-effort display/export redaction, not a DLP guarantee.
#[derive(Clone, Default)]
pub struct Redactor {
    secrets: Vec<String>,
    paths: Vec<String>,
}
impl Redactor {
    pub fn with_secrets(secrets: Vec<String>) -> Self {
        Self {
            secrets: secrets.into_iter().filter(|s| !s.is_empty()).collect(),
            paths: vec![],
        }
    }
    pub fn contains_secret(&self, text: &str) -> bool {
        self.secrets
            .iter()
            .chain(crate::credentials::loaded_secrets().iter())
            .any(|s| text.contains(s))
    }
    pub fn environment(workspace: &str) -> Self {
        let mut secrets: Vec<String> = std::env::vars()
            .filter(|(k, v)| {
                v.len() >= 8
                    && (k.contains("KEY")
                        || k.contains("TOKEN")
                        || k.contains("SECRET")
                        || k.contains("PASSWORD"))
            })
            .map(|(_, v)| v)
            .collect();
        secrets.extend(crate::credentials::loaded_secrets());
        let mut paths = vec![workspace.to_owned()];
        if let Ok(home) = std::env::var("HOME") {
            paths.push(home);
        }
        paths.retain(|p| p.len() > 1);
        paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
        Self { secrets, paths }
    }
    pub fn text(&self, input: &str) -> String {
        let mut s: String = input
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();
        for secret in self
            .secrets
            .iter()
            .chain(crate::credentials::loaded_secrets().iter())
        {
            s = s.replace(secret, "[REDACTED]");
        }
        for path in &self.paths {
            s = s.replace(path, "[LOCAL_PATH]");
        }
        s
    }
    pub fn value(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => self.text(s).into(),
            serde_json::Value::Array(a) => a.iter().map(|v| self.value(v)).collect(),
            serde_json::Value::Object(o) => o
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if [
                            "api_key",
                            "access_token",
                            "refresh_token",
                            "authorization",
                            "authUrl",
                        ]
                        .iter()
                        .any(|x| k.eq_ignore_ascii_case(x))
                        {
                            "[REDACTED]".into()
                        } else {
                            self.value(v)
                        },
                    )
                })
                .collect(),
            _ => value.clone(),
        }
    }
}

/// Retains enough unrendered text to redact a known value spanning stream chunks.
pub struct StreamRedactor {
    buffer: String,
    redactor: Redactor,
}
impl StreamRedactor {
    pub fn new(redactor: Redactor) -> Self {
        Self {
            buffer: String::new(),
            redactor,
        }
    }
    pub fn push(&mut self, text: &str) -> String {
        self.buffer.push_str(text);
        let values = self
            .redactor
            .secrets
            .iter()
            .chain(self.redactor.paths.iter());
        let window = values.clone().map(String::len).max().unwrap_or(1);
        let mut end = self.buffer.len().saturating_sub(window);
        while !self.buffer.is_char_boundary(end) {
            end -= 1;
        }
        for value in values {
            for (start, _) in self.buffer.match_indices(value) {
                if start < end && start + value.len() > end {
                    end = start;
                }
            }
        }
        let ready = self.buffer[..end].to_owned();
        self.buffer.drain(..end);
        self.redactor.text(&ready)
    }
    pub fn finish(&mut self) -> String {
        let text = self.redactor.text(&self.buffer);
        self.buffer.clear();
        text
    }
}
