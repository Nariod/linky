use chrono::{DateTime, Local};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::tasks::{Task, TaskStatus};

#[derive(Debug, Clone, PartialEq)]
pub enum LinkStatus {
    Active,
    Inactive,
    Exited,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub id: Uuid,
    pub name: String,
    pub status: LinkStatus,
    pub platform: String,
    pub username: String,
    pub hostname: String,
    pub internal_ip: String,
    /// Stored for potential future use (e.g. display, logging).
    #[allow(dead_code)]
    pub external_ip: String,
    pub pid: u32,
    pub first_checkin: DateTime<Local>,
    pub last_checkin: DateTime<Local>,
    /// Changes on every check-in; used to correlate requests.
    pub x_request_id: Uuid,
    pub tasks: VecDeque<Task>,
}

#[derive(Default)]
pub struct Links {
    pub links: Vec<Link>,
    counter: usize,
}

impl Links {
    pub fn add_link(
        &mut self,
        username: String,
        hostname: String,
        internal_ip: String,
        external_ip: String,
        platform: String,
        pid: u32,
    ) -> &Link {
        let now = Local::now();
        self.counter += 1;
        self.links.push(Link {
            id: Uuid::new_v4(),
            name: format!("link-{}", self.counter),
            status: LinkStatus::Active,
            platform,
            username,
            hostname,
            internal_ip,
            external_ip,
            pid,
            first_checkin: now,
            last_checkin: now,
            x_request_id: Uuid::new_v4(),
            tasks: VecDeque::new(),
        });
        self.links.last().unwrap()
    }

    pub fn find_by_request_id(&self, x_request_id: Uuid) -> Option<&Link> {
        self.links
            .iter()
            .find(|l| l.x_request_id == x_request_id && l.status != LinkStatus::Exited)
    }

    pub fn update_checkin(&mut self, link_id: Uuid, new_x_request_id: Uuid) {
        if let Some(link) = self.links.iter_mut().find(|l| l.id == link_id) {
            link.last_checkin = Local::now();
            link.x_request_id = new_x_request_id;
            link.status = LinkStatus::Active;
        }
    }

    /// Returns the first `Waiting` task and marks it `InProgress`.
    pub fn get_next_task(&mut self, link_id: Uuid) -> Option<Task> {
        let link = self.links.iter_mut().find(|l| l.id == link_id)?;
        let task = link
            .tasks
            .iter_mut()
            .find(|t| t.status == TaskStatus::Waiting)?;
        let snapshot = task.clone();
        task.status = TaskStatus::InProgress;
        Some(snapshot)
    }

    pub fn complete_task(&mut self, link_id: Uuid, task_id: Uuid, output: String) {
        let Some(link) = self.links.iter_mut().find(|l| l.id == link_id) else {
            return;
        };
        if let Some(task) = link.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = TaskStatus::Completed;
            task.output = output;
        }
    }

    pub fn add_task(
        &mut self,
        link_id: Uuid,
        command: String,
        cli_command: String,
    ) -> Option<Uuid> {
        let link = self.links.iter_mut().find(|l| l.id == link_id)?;
        let task = Task::new(command, cli_command);
        let id = task.id;
        link.tasks.push_back(task);
        Some(id)
    }

    /// Mark links that have not checked in for more than 90 s as `Inactive`.
    pub fn mark_inactive(&mut self) {
        let threshold = Local::now() - chrono::TimeDelta::seconds(90);
        for link in &mut self.links {
            if link.status == LinkStatus::Active && link.last_checkin < threshold {
                link.status = LinkStatus::Inactive;
            }
        }
    }

    pub fn all_links(&self) -> &[Link] {
        &self.links
    }

    pub fn get_link(&self, id: Uuid) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn get_link_by_name(&self, name: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.name == name)
    }

    pub fn kill_link(&mut self, link_id: Uuid) {
        if let Some(link) = self.links.iter_mut().find(|l| l.id == link_id) {
            link.status = LinkStatus::Exited;
        }
    }
}
