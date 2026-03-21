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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_links() -> Links {
        Links::default()
    }

    fn add_test_link(links: &mut Links, platform: &str) -> Uuid {
        let link = links.add_link(
            "user".into(),
            "host".into(),
            "10.0.0.1".into(),
            "1.2.3.4".into(),
            platform.into(),
            1234,
        );
        link.id
    }

    #[test]
    fn add_and_get_link() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        let link = links.get_link(id).expect("link should exist");
        assert_eq!(link.platform, "linux");
        assert_eq!(link.status, LinkStatus::Active);
    }

    #[test]
    fn link_names_are_sequential() {
        let mut links = make_links();
        let id1 = add_test_link(&mut links, "linux");
        let id2 = add_test_link(&mut links, "windows");
        assert_eq!(links.get_link(id1).unwrap().name, "link-1");
        assert_eq!(links.get_link(id2).unwrap().name, "link-2");
    }

    #[test]
    fn get_link_by_name() {
        let mut links = make_links();
        add_test_link(&mut links, "linux");
        let link = links
            .get_link_by_name("link-1")
            .expect("should find by name");
        assert_eq!(link.platform, "linux");
    }

    #[test]
    fn get_link_by_name_not_found() {
        let mut links = make_links();
        assert!(links.get_link_by_name("nonexistent").is_none());
    }

    #[test]
    fn find_by_request_id() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        let x_req_id = links.get_link(id).unwrap().x_request_id;
        let found = links
            .find_by_request_id(x_req_id)
            .expect("should find by request id");
        assert_eq!(found.id, id);
    }

    #[test]
    fn find_by_request_id_not_found_after_exit() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        let x_req_id = links.get_link(id).unwrap().x_request_id;
        links.kill_link(id);
        assert!(links.find_by_request_id(x_req_id).is_none());
    }

    #[test]
    fn kill_link_sets_exited() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        links.kill_link(id);
        assert_eq!(links.get_link(id).unwrap().status, LinkStatus::Exited);
    }

    #[test]
    fn add_task_and_get_next() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        links.add_task(id, "whoami".into(), "whoami".into());
        let task = links.get_next_task(id).expect("should have a task");
        assert_eq!(task.command, "whoami");
    }

    #[test]
    fn get_next_task_returns_none_when_empty() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        assert!(links.get_next_task(id).is_none());
    }

    #[test]
    fn complete_task_stores_output() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        let task_id = links
            .add_task(id, "whoami".into(), "whoami".into())
            .unwrap();
        // Fetch to move it to InProgress
        links.get_next_task(id);
        links.complete_task(id, task_id, "root".into());
        let link = links.get_link(id).unwrap();
        let task = link.tasks.iter().find(|t| t.id == task_id).unwrap();
        assert_eq!(task.output, "root");
        assert_eq!(task.status, crate::tasks::TaskStatus::Completed);
    }

    #[test]
    fn mark_inactive_updates_stale_links() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        // Force last_checkin to be far in the past
        let link = links.links.iter_mut().find(|l| l.id == id).unwrap();
        link.last_checkin = Local::now() - chrono::TimeDelta::seconds(120);
        links.mark_inactive();
        assert_eq!(links.get_link(id).unwrap().status, LinkStatus::Inactive);
    }

    #[test]
    fn mark_inactive_does_not_affect_exited_links() {
        let mut links = make_links();
        let id = add_test_link(&mut links, "linux");
        links.kill_link(id);
        let link = links.links.iter_mut().find(|l| l.id == id).unwrap();
        link.last_checkin = Local::now() - chrono::TimeDelta::seconds(120);
        links.mark_inactive();
        // Should remain Exited, not become Inactive
        assert_eq!(links.get_link(id).unwrap().status, LinkStatus::Exited);
    }
}
