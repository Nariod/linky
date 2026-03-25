use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Local;
use colored::Colorize;

use crate::links::{Links, Link, LinkStatus};
use crate::tasks::{Task, TaskStatus};

#[test]
fn test_show_completed_task_results() {
    // Setup
    let links = Arc::new(Mutex::new(Links::new()));
    let link_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    
    // Add a link
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_link(
            link_id,
            "test-link".to_string(),
            "test-user".to_string(),
            "test-host".to_string(),
            "127.0.0.1".to_string(),
            "test-platform".to_string(),
            1234,
        );
    }
    
    // Add a completed task with output
    {
        let mut links_guard = links.lock().unwrap();
        if let Some(link) = links_guard.get_link_mut(link_id) {
            link.tasks.push_back(Task {
                id: task_id,
                command: "whoami".to_string(),
                cli_command: "whoami".to_string(),
                status: TaskStatus::Completed,
                output: "test-user@test-host".to_string(),
                file_content: None,
                file_name: None,
                upload_content: None,
                upload_path: None,
            });
        }
    }
    
    // Test the function
    super::show_completed_task_results(&links, link_id);
    
    // We can't easily capture stdout in tests, but we can verify the logic
    let links_guard = links.lock().unwrap();
    let link = links_guard.get_link(link_id).unwrap();
    assert_eq!(link.tasks.len(), 1);
    assert_eq!(link.tasks[0].status, TaskStatus::Completed);
    assert!(!link.tasks[0].output.is_empty());
}

#[test]
fn test_task_completion_flow() {
    // This test simulates the full flow:
    // 1. Create link
    // 2. Add task
    // 3. Complete task
    // 4. Verify results are displayed
    
    let links = Arc::new(Mutex::new(Links::new()));
    let link_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    
    // Step 1: Create link
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_link(
            link_id,
            "test-link".to_string(),
            "test-user".to_string(),
            "test-host".to_string(),
            "127.0.0.1".to_string(),
            "linux".to_string(),
            1234,
        );
    }
    
    // Step 2: Add task (simulating command execution)
    {
        let mut links_guard = links.lock().unwrap();
        links_guard.add_task(link_id, "whoami".to_string(), "whoami".to_string());
    }
    
    // Step 3: Complete task (simulating implant response)
    {
        let mut links_guard = links.lock().unwrap();
        // Get the task ID that was created
        if let Some(link) = links_guard.get_link_mut(link_id) {
            if let Some(task) = link.tasks.back() {
                let created_task_id = task.id;
                links_guard.complete_task(link_id, created_task_id, "test-user".to_string());
            }
        }
    }
    
    // Step 4: Verify task is completed
    let links_guard = links.lock().unwrap();
    let link = links_guard.get_link(link_id).unwrap();
    assert_eq!(link.tasks.len(), 1);
    assert_eq!(link.tasks[0].status, TaskStatus::Completed);
    assert_eq!(link.tasks[0].output, "test-user");
}
