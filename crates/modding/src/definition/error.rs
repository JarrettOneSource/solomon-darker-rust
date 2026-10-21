//! Source definition diagnostics from `web-lua-definition-error.ts`.

use std::fmt;

use solomon_sim::modding::definition::WebLuaDefinitionSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebLuaDefinitionErrorCode {
    EApiVersion,
    EAsset,
    EBudget,
    EContentKey,
    ECycle,
    EDuplicate,
    EGraph,
    EMountConflict,
    EReference,
    ESchema,
    EScript,
    EUnknownField,
}

impl WebLuaDefinitionErrorCode {
    pub const ALL: [Self; 12] = [
        Self::EApiVersion,
        Self::EAsset,
        Self::EBudget,
        Self::EContentKey,
        Self::ECycle,
        Self::EDuplicate,
        Self::EGraph,
        Self::EMountConflict,
        Self::EReference,
        Self::ESchema,
        Self::EScript,
        Self::EUnknownField,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EApiVersion => "E_API_VERSION",
            Self::EAsset => "E_ASSET",
            Self::EBudget => "E_BUDGET",
            Self::EContentKey => "E_CONTENT_KEY",
            Self::ECycle => "E_CYCLE",
            Self::EDuplicate => "E_DUPLICATE",
            Self::EGraph => "E_GRAPH",
            Self::EMountConflict => "E_MOUNT_CONFLICT",
            Self::EReference => "E_REFERENCE",
            Self::ESchema => "E_SCHEMA",
            Self::EScript => "E_SCRIPT",
            Self::EUnknownField => "E_UNKNOWN_FIELD",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WebLuaDefinitionIssue {
    pub code: WebLuaDefinitionErrorCode,
    pub message: String,
    pub path: String,
    pub source: Option<WebLuaDefinitionSource>,
}

impl WebLuaDefinitionIssue {
    pub fn new(
        code: WebLuaDefinitionErrorCode,
        path: impl Into<String>,
        message: impl Into<String>,
        source: Option<&WebLuaDefinitionSource>,
    ) -> Self {
        Self { code, message: message.into(), path: path.into(), source: source.cloned() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebLuaDefinitionError {
    pub issues: Vec<WebLuaDefinitionIssue>,
}

impl WebLuaDefinitionError {
    pub fn new(issues: Vec<WebLuaDefinitionIssue>) -> Self {
        assert!(!issues.is_empty(), "WebLuaDefinitionError requires at least one issue");
        Self { issues }
    }
}

impl fmt::Display for WebLuaDefinitionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, issue) in self.issues.iter().enumerate() {
            if index > 0 {
                writeln!(output)?;
            }
            write!(output, "{} {}", issue.code.as_str(), issue.path)?;
            if let Some(source) = &issue.source {
                write!(output, " {}", source.file)?;
                if let Some(line) = source.line.filter(|line| *line > 0) {
                    write!(output, ":{line}")?;
                }
            }
            write!(output, ": {}", issue.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for WebLuaDefinitionError {}
