use context::prelude::ErrorContext;
use context::prelude::*;

pub type Result<T> = std::result::Result<T, LightFieldError>;

#[derive(Debug)]
pub enum LightFieldError {
    ContextError(ContextError),
    LightFieldLoader(ErrorContext<String>),
    ConfigLoader(ErrorContext<String>),
}

impl LightFieldError {
    pub fn light_field_loader(msg: &str) -> Self {
        LightFieldError::LightFieldLoader(ErrorContext::new(msg.to_string()))
    }

    pub fn config_loader(msg: &str) -> Self {
        LightFieldError::ConfigLoader(ErrorContext::new(msg.to_string()))
    }
}

impl std::fmt::Display for LightFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightFieldError::ContextError(err) => err.fmt(f),
            LightFieldError::LightFieldLoader(err) => err.fmt(f),
            LightFieldError::ConfigLoader(err) => err.fmt(f),
        }
    }
}

impl Fail for LightFieldError {
    fn cause(&self) -> Option<&dyn Fail> {
        match self {
            LightFieldError::ContextError(err) => err.cause(),
            LightFieldError::LightFieldLoader(err) => err.cause(),
            LightFieldError::ConfigLoader(err) => err.cause(),
        }
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        match self {
            LightFieldError::ContextError(err) => err.backtrace(),
            LightFieldError::LightFieldLoader(err) => err.backtrace(),
            LightFieldError::ConfigLoader(err) => err.backtrace(),
        }
    }
}

impl From<ContextError> for LightFieldError {
    fn from(err: ContextError) -> Self {
        LightFieldError::ContextError(err)
    }
}

impl From<PresentationError> for LightFieldError {
    fn from(error: PresentationError) -> Self {
        LightFieldError::ContextError(ContextError::from(error))
    }
}

impl From<UtilError> for LightFieldError {
    fn from(error: UtilError) -> Self {
        LightFieldError::ConfigLoader(ErrorContext::new(format!("{}", error)))
    }
}

impl From<VulkanError> for LightFieldError {
    fn from(error: VulkanError) -> Self {
        LightFieldError::ContextError(ContextError::from(error))
    }
}
