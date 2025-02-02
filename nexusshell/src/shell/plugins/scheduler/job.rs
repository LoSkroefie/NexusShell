use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, Duration};
use tokio::sync::mpsc;
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::Result;
use cron::Schedule;
use std::str::FromStr;
use tokio::time;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobSchedule {
    Once(DateTime<Utc>),
    Recurring(String), // Cron expression
    Interval(Duration),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub run_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub schedule: JobSchedule,
    pub status: JobStatus,
    pub metadata: JobMetadata,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    pub timeout: Option<Duration>,
    pub retry_count: u32,
    pub retry_delay: Duration,
    pub dependencies: Vec<String>,
}

impl Job {
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        schedule: JobSchedule,
        env: HashMap<String, String>,
        working_dir: Option<String>,
        timeout: Option<Duration>,
        retry_count: u32,
        retry_delay: Duration,
        dependencies: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        let next_run = match &schedule {
            JobSchedule::Once(time) => Some(*time),
            JobSchedule::Recurring(cron_expr) => {
                Schedule::from_str(cron_expr)
                    .ok()
                    .and_then(|schedule| schedule.upcoming(Utc).next())
            }
            JobSchedule::Interval(_) => Some(now),
        };

        Job {
            id: Uuid::new_v4().to_string(),
            name,
            command,
            args,
            schedule,
            status: JobStatus::Pending,
            metadata: JobMetadata {
                created_at: now,
                updated_at: now,
                last_run: None,
                next_run,
                run_count: 0,
            },
            env,
            working_dir,
            timeout,
            retry_count,
            retry_delay,
            dependencies,
        }
    }

    pub fn update_status(&mut self, status: JobStatus) {
        self.status = status;
        self.metadata.updated_at = Utc::now();
    }

    pub fn update_next_run(&mut self) {
        let now = Utc::now();
        self.metadata.next_run = match &self.schedule {
            JobSchedule::Once(_) => None,
            JobSchedule::Recurring(cron_expr) => {
                Schedule::from_str(cron_expr)
                    .ok()
                    .and_then(|schedule| schedule.upcoming(Utc).next())
            }
            JobSchedule::Interval(duration) => {
                Some(now + *duration)
            }
        };
    }

    pub async fn execute(&mut self, tx: mpsc::Sender<JobResult>) -> Result<()> {
        let now = Utc::now();
        self.metadata.last_run = Some(now);
        self.metadata.run_count += 1;
        self.update_status(JobStatus::Running);

        let mut command = tokio::process::Command::new(&self.command);
        command.args(&self.args);
        command.envs(&self.env);

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }

        let mut retry_count = 0;
        let result = loop {
            match command.output().await {
                Ok(output) => {
                    if output.status.success() {
                        break JobResult {
                            job_id: self.id.clone(),
                            success: true,
                            output: String::from_utf8_lossy(&output.stdout).to_string(),
                            error: None,
                            exit_code: output.status.code(),
                            completed_at: Utc::now(),
                        };
                    } else {
                        let error = String::from_utf8_lossy(&output.stderr).to_string();
                        if retry_count < self.retry_count {
                            retry_count += 1;
                            time::sleep(self.retry_delay).await;
                            continue;
                        }
                        break JobResult {
                            job_id: self.id.clone(),
                            success: false,
                            output: String::from_utf8_lossy(&output.stdout).to_string(),
                            error: Some(error),
                            exit_code: output.status.code(),
                            completed_at: Utc::now(),
                        };
                    }
                }
                Err(e) => {
                    if retry_count < self.retry_count {
                        retry_count += 1;
                        time::sleep(self.retry_delay).await;
                        continue;
                    }
                    break JobResult {
                        job_id: self.id.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                        exit_code: None,
                        completed_at: Utc::now(),
                    };
                }
            }
        };

        self.update_status(if result.success {
            JobStatus::Completed
        } else {
            JobStatus::Failed(result.error.unwrap_or_default())
        });

        self.update_next_run();
        tx.send(result).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFilter {
    pub status: Option<JobStatus>,
    pub name: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub command: Option<String>,
}

impl JobFilter {
    pub fn matches(&self, job: &Job) -> bool {
        if let Some(status) = &self.status {
            if !matches!((status, &job.status), 
                (JobStatus::Pending, JobStatus::Pending) |
                (JobStatus::Running, JobStatus::Running) |
                (JobStatus::Completed, JobStatus::Completed) |
                (JobStatus::Cancelled, JobStatus::Cancelled) |
                (JobStatus::Failed(_), JobStatus::Failed(_))) {
                return false;
            }
        }

        if let Some(name) = &self.name {
            if !job.name.contains(name) {
                return false;
            }
        }

        if let Some(created_after) = self.created_after {
            if job.metadata.created_at < created_after {
                return false;
            }
        }

        if let Some(created_before) = self.created_before {
            if job.metadata.created_at > created_before {
                return false;
            }
        }

        if let Some(command) = &self.command {
            if !job.command.contains(command) {
                return false;
            }
        }

        true
    }
}
