//! Real Lua definition execution and immutable graph preparation.
//! Source: Website `modding/definition/`.

pub mod assembly;
pub mod compiler;
pub mod error;
pub mod runtime;
mod schemas;
pub mod scripts;
mod suggestions;
pub(crate) mod values;

pub(crate) use schemas::content_schema;

pub use compiler::{
    CompileWebLuaDefinitionOptions, WebLuaDefinitionDependency, WebLuaDefinitionDependencyContent,
    compile_web_lua_definition,
};
pub use error::{WebLuaDefinitionError, WebLuaDefinitionErrorCode, WebLuaDefinitionIssue};
