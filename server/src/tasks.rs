use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Waiting,
    InProgress,
    Completed,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: Uuid,
    pub command: String,
    /// Original CLI string as typed by the operator (stored for audit/display).
    #[allow(dead_code)]
    pub cli_command: String,
    pub status: TaskStatus,
    pub output: String,
}

impl Task {
    pub fn new(command: String, cli_command: String) -> Self {
        Task {
            id: Uuid::new_v4(),
            command,
            cli_command,
            status: TaskStatus::Waiting,
            output: String::new(),
        }
    }
}
