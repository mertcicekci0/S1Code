//! Best-effort display/export redaction, not a DLP guarantee.
#[derive(Clone, Default)]
pub struct Redactor {
    secrets: Vec<String>,
    paths: Vec<String>,
}
impl Redactor {
    pub fn environment(workspace: &str) -> Self {
        let secrets = std::env::vars()
            .filter(|(k, v)| {
                v.len() >= 8
                    && (k.contains("KEY")
                        || k.contains("TOKEN")
                        || k.contains("SECRET")
                        || k.contains("PASSWORD"))
            })
            .map(|(_, v)| v)
            .collect();
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
        for secret in &self.secrets {
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
