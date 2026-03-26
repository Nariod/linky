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
    pub cli_command: String,
    pub status: TaskStatus,
    pub output: String,
    pub displayed: bool,
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
            displayed: false,
            file_content: None,
            file_name: None,
            upload_content: None,
            upload_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_is_waiting() {
        let t = Task::new("whoami".into(), "whoami".into());
        assert_eq!(t.status, TaskStatus::Waiting);
        assert!(t.output.is_empty());
        assert!(t.file_content.is_none());
        assert!(t.upload_content.is_none());
    }

    #[test]
    fn task_has_unique_ids() {
        let t1 = Task::new("cmd1".into(), "cmd1".into());
        let t2 = Task::new("cmd2".into(), "cmd2".into());
        assert_ne!(t1.id, t2.id);
    }
}
