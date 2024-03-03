use std::fmt;

const USAGE: &str = r#"
Usage: %s {COMMAND} ...
COMMANDS:
wait         wait for the specified objects to appear on the DBus
subtree-remove
             wait until the specified interface is not present
             in any of the subtrees of the specified namespace
get-service  return the service identifier for input path"#;

#[derive(Debug, Clone)]
pub enum Error {
    MissingCommand,
    InvalidCommand,
    MissingWaitArg(String),
    MissingSubtreeRemoveArg(String),
    InvalidSubtreeRemoveArg(String),
    MissingGetServiceArg(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MissingCommand => write!(f, "{}", USAGE),
            Error::InvalidCommand => write!(f, "{}", USAGE),
            Error::MissingWaitArg(command) => write!(f, "Usage: {} wait OBJECTPATH\n", command),
            Error::MissingSubtreeRemoveArg(command) => {
                write!(f, "Usage: {} subtree-remove NAMESPACE:INTERFACE\n", command)
            }
            Error::InvalidSubtreeRemoveArg(namespace_interface) => {
                write!(f, "Token ':' was not found in '{}'\n", namespace_interface)
            }
            Error::MissingGetServiceArg(command) => {
                write!(f, "Usage: {} get-service OBJECTPATH\n", command)
            }
        }
    }
}

impl std::error::Error for Error {}
