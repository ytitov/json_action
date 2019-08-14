use serde_json::Error as JsonError;
use std::error;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionError {
    pub code: String,
    pub message: String,
}

impl ActionError {
    pub fn new(code: &str, message: &str) -> Self {
        ActionError {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ActionError. Code: {}  Message: {}",
            self.code, self.message
        )
    }
}

impl error::Error for ActionError {
    fn description(&self) -> &str {
        &self.message
    }
}

pub trait ToActionError {
    fn to_action_error(&self) -> ActionError;
}

impl ToActionError for serde_json::Error {
    fn to_action_error(&self) -> ActionError {
        ActionError::new("SerdeJson", &self.to_string())
    }
}

impl ToActionError for ActionError {
    fn to_action_error(&self) -> ActionError {
        ActionError::new(&self.code, &self.message)
    }
}

impl ToActionError for (String, String) {
    fn to_action_error(&self) -> ActionError {
        ActionError::new(&self.0, &self.1)
    }
}

impl From<JsonError> for ActionError {
    fn from(error: JsonError) -> Self {
        ActionError::new("JsonError", &error.to_string())
    }
}

impl From<(String, String)> for ActionError {
    fn from((a, b): (String, String)) -> ActionError {
        ActionError::new(&a, &b)
    }
}

impl From<std::io::Error> for ActionError {
    fn from(error: std::io::Error) -> ActionError {
        ActionError::new("io::Error", &error.to_string())
    }
}

impl From<Box<std::error::Error>> for ActionError {
    fn from(error: Box<std::error::Error>) -> ActionError {
        // TODO: get the cause to display better
        ActionError::new("Boxed::Error", &error.to_string())
    }
}
