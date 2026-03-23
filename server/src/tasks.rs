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
    /// For file download tasks, contains the base64 encoded file content
    pub file_content: Option<String>,
    /// For file download tasks, contains the original file name
    pub file_name: Option<String>,
    /// For file upload tasks, contains the base64 encoded file content to upload
    pub upload_content: Option<String>,
    /// For file upload tasks, contains the destination path
    pub upload_path: Option<String>,
}

impl Task {
    pub fn new(command: String, cli_command: String) -> Self {
        Task {
            id: Uuid::new_v4(),
            command,
            cli_command,
            status: TaskStatus::Waiting,
            output: String::new(),
            file_content: None,
            file_name: None,
            upload_content: None,
            upload_path: None,
        }
    }
}
