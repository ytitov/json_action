use bytes::Bytes;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

//use serde::de::DeserializeOwned;
use serde::de::Deserialize;

use crate::error::{ActionError, ToActionError};

pub type ActionHandler<R> = Fn(&R, &Action) -> Result<serde_json::Value, ActionError> + 'static;
pub type ManagerInitHandler<R> = Fn(&R) -> Result<(), ActionError>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    // this determines which handler (closure) will run and work with the action
    pub name: String,
    // this is assumed to be a unique id, for the benefit of the client
    // when they get a response because they're always connected and
    // it is assumed they will request to do many actions and ordering of the
    // replies is not guaranteed because of ..async
    pub id: u64,
    /// unique token attributable to a specific user
    pub token: Option<String>,
    /// arbitrary binary data if not using binary
    pub base64: Option<String>,
    pub payload: HashMap<String, Value>,
    // the output of the action
    pub result: Option<Value>,
    // the error message, setting this thing sets is_ok to false
    pub errors: Option<Vec<ActionError>>,
}

#[derive(Serialize, Deserialize)]
pub struct ActionReply {
    pub id: u64,
    //#[serde(borrow)]
    pub name: String,
    pub payload: HashMap<String, Value>,
    pub result: Option<Value>,
    // this should always be available in the action
    pub errors: Vec<ActionError>,
}

pub fn try_action<V, E>(v: Result<V, E>) -> Result<serde_json::Value, ActionError>
where
    V: Serialize,
    E: ToActionError,
{
    match v {
        Ok(val) => {
            let v = serde_json::to_value(&val).expect("try_action, serde_json::to_value blew up");
            Ok(v)
        }
        Err(e) => Err(e.to_action_error()),
    }
}

pub fn value_ok<V>(v: V) -> Result<serde_json::Value, ActionError>
where
    V: Serialize,
{
    match serde_json::to_value(&v) {
        Ok(val) => Ok(val),
        Err(e) => Err(ActionError::new("ToValue", &e.to_string())),
    }
}

pub fn value_err<E: std::error::Error>(name: &str, e: E) -> Result<serde_json::Value, ActionError> {
    Err(ActionError::new(name, &e.to_string()))
}

pub fn action_ok() -> Result<serde_json::Value, ActionError> {
    let v = json!({"success": true});
    Ok(v)
}

impl Action {
    pub fn set_result(&mut self, res: Value) {
        //println!("Action.set_result {:?}", res);
        self.result = Some(res);
    }

    pub fn set_error(&mut self, value: ActionError) {
        match &mut self.errors {
            Some(v) => v.push(value),
            None => self.errors = Some(vec![value]),
        };
    }

    pub fn from_payload<Q>(&self) -> Result<Q, ActionError>
    where
        for<'de> Q: Deserialize<'de>,
    {
        let o = serde_json::to_value(&self.payload).unwrap();
        match serde_json::from_value::<Q>(o) {
            Ok(v) => Ok(v),
            Err(e) => Err(ActionError::new("PayloadError", &e.to_string())),
        }
    }

    pub fn from_result<Q>(&self) -> Result<Q, ActionError>
    where
        for<'de> Q: Deserialize<'de>,
    {
        let o = serde_json::to_value(&self.result).unwrap();
        match serde_json::from_value::<Q>(o) {
            Ok(v) => Ok(v),
            Err(e) => Err(ActionError::new("PayloadError", &e.to_string())),
        }
    }

    pub fn from_bytes(buf: Bytes) -> Result<Self, String> {
        // TODO: this can panic, so need to handle it
        let jsonstr = std::str::from_utf8(&buf).unwrap();
        let action: Result<Action, String> = match serde_json::from_str(jsonstr) {
            Ok(a) => Ok(a),
            Err(e) => Err(e.to_string()),
        };
        action
    }

    pub fn server_err(err: ActionError) -> Self {
        let mut v = Vec::new();
        v.push(err);
        Action {
            id: 0,
            token: None,
            name: "server-error".to_owned(),
            base64: None,
            payload: HashMap::new(),
            errors: Some(v),
            result: None,
        }
    }

    pub fn into(&self) -> Self {
        Action {
            id: 0,
            token: None,
            name: "server-error".to_owned(),
            base64: None,
            payload: HashMap::new(),
            errors: None,
            result: None,
        }
    }

    pub fn into_reply(self) -> ActionReply {
        let errors = match self.errors {
            Some(e) => e,
            None => Vec::new(),
        };
        ActionReply {
            id: self.id,
            name: self.name,
            payload: self.payload,
            result: self.result,
            errors,
        }
    }
}

pub struct ManagerFut<R> {
    // contains a map of closures
    // the return value at this point is not used... should just get rid of it
    // I don't know...
    //actions: HashMap<String, Box<Fn(&R, &Action) -> Result<serde_json::Value, ActionError>>>,
    name: String,
    actions: HashMap<String, Box<Fn(&R, &Action) -> Result<(), ActionError> + 'static>>,
    resource: R,
}

impl<R> ManagerFut<R> {
    pub fn new(name: &str, resource: R) -> Self {
        ManagerFut {
            name: name.to_owned(),
            actions: HashMap::new(),
            resource,
        }
    }
    /// identical to action but this is syntactically better to use a little bit
    pub fn on<T>(&mut self, name: &str, f: T)
    where
        T: Fn(&R, &Action) -> Result<(), ActionError> + 'static,
    {
        if self.actions.contains_key(name) {
            println!(
                "WARNING: Manager [{:}] registered existing action: {:}, ignoring",
                self.name, name
            );
        } else {
            println!("Manager [{:}] register action: {}", self.name, name);
            self.actions.insert(name.to_owned(), Box::new(f));
        }
    }
}

pub struct Manager<R> {
    // contains a map of closures
    // the return value at this point is not used... should just get rid of it
    // I don't know...
    //actions: HashMap<String, Box<Fn(&R, &Action) -> Result<serde_json::Value, ActionError>>>,
    name: String,
    actions: HashMap<String, Box<ActionHandler<R>>>,
    resource: Option<R>,
    gen_resource: Option<Box<Fn() -> R>>,
}

impl<R> Manager<R> {
    pub fn new(name: &str, resource: R) -> Self {
        Manager {
            name: name.to_owned(),
            actions: HashMap::new(),
            resource: Some(resource),
            gen_resource: None,
        }
    }

    pub fn with<T>(name: &str, f: T) -> Self
    where
        T: Fn() -> R + 'static,
    {
        Manager {
            name: name.to_owned(),
            actions: HashMap::new(),
            resource: None,
            gen_resource: Some(Box::new(f)),
        }
    }

    pub fn init(&mut self, f: &'static ManagerInitHandler<R>) {
        if let Some(r) = &self.resource {
            match f(&r) {
                Ok(_) => (),
                Err(e) => panic!("Error during init {:?}", e),
            }
        }
        if let Some(gen_resource) = &self.gen_resource {
            let r = gen_resource();
            match f(&r) {
                Ok(_) => (),
                Err(e) => panic!("Error during init {:?}", e),
            }
        }
    }

    pub fn action(&mut self, name: &str, f: &'static ActionHandler<R>) {
        if self.actions.contains_key(name) {
            println!(
                "WARNING: Manager [{:}] registered existing action: {:}, ignoring",
                self.name, name
            );
        } else {
            println!("Manager [{:}] register action: {}", self.name, name);
            self.actions.insert(name.to_owned(), Box::new(f));
        }
    }

    //pub fn for_each<T> (&mut self, f: T) where T: Fn(&Q) -> R + 'static {
    pub fn for_each<T>(&mut self, f: T)
    where
        T: Fn() -> R + 'static,
    {
        self.gen_resource = Some(Box::new(f));
    }

    /// identical to action but this is syntactically better to use a little bit
    pub fn on<T>(&mut self, name: &str, f: T)
    where
        T: Fn(&R, &Action) -> Result<serde_json::Value, ActionError> + 'static,
    {
        if self.actions.contains_key(name) {
            println!(
                "WARNING: Manager [{:}] registered existing action: {:}, ignoring",
                self.name, name
            );
        } else {
            println!("Manager [{:}] register on: {}", self.name, name);
            self.actions.insert(name.to_owned(), Box::new(f));
        }
    }

    pub fn do_action(&self, action: &mut Action) {
        if let Some(gen_resource) = &self.gen_resource {
            let r = gen_resource();
            self.run_action(&r, action);
        } else {
            //println!("executing action {:?}", action.name);
            if let Some(r) = &self.resource {
                self.run_action(&r, action);
            }
        };
    }

    fn run_action(&self, resource: &R, action: &mut Action) {
        match self.actions.get(&action.name) {
            Some(func) => {
                match func(resource, &action) {
                    Ok(v) => {
                        //println!("func returned some result {:?}",v);
                        action.set_result(serde_json::value::to_value(&v)
                                          .expect("Fatal error, some function returned something that can't be converted to a json value"))
                    }
                    Err(e) => action.set_error(e),
                };
            }
            _ => {
                // reply with an error, cuz action was not found
                action.set_error(ActionError::new(
                    &format!("{:} - DoAction", self.name),
                    "Action does NOT exist, make sure it is valid",
                ));
            }
        };
    }

    pub fn do_action_if_exists(&self, action: &mut Action) {
        match self.actions.get(&action.name) {
            Some(func) => {
                //println!("executing action {:?}", action.name);
                if let Some(r) = &self.resource {
                    match func(&r, &action) {
                        Ok(v) => {
                            //println!("func returned some result {:?}",v);
                            action.set_result(serde_json::value::to_value(&v)
                                              .expect("Fatal error, some function returned something that can't be converted to a json value"))
                        }
                        Err(e) => action.set_error(e),
                    };
                };
                if let Some(gen_resource) = &self.gen_resource {
                    let r = gen_resource();
                    self.run_action(&r, action);
                };
            }
            _ => {
                // reply with an error, cuz action was not found
                //action.set_error(ActionError::new("DoAction", "Action does NOT exist, make sure it is valid"));
            }
        };
    }
}
