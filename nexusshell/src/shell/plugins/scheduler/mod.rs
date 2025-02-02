mod job;
mod queue;

use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use job::{Job, JobSchedule, JobStatus, JobFilter};
use queue::{JobQueue, QueueConfig};
use std::collections::HashMap;
use colored::*;
use std::str::FromStr;
use tokio::fs;
use std::path::PathBuf;

pub struct SchedulerPlugin {
    queue: JobQueue,
}

impl SchedulerPlugin {
    pub async fn new() -> Result<Self> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("scheduler");

        let config = QueueConfig {
            storage_path: config_path,
            ..Default::default()
        };

        let queue = JobQueue::new(config).await?;
        let scheduler = SchedulerPlugin { queue };

        // Start the job processing loop
        let queue_clone = scheduler.queue.clone();
        tokio::spawn(async move {
            queue_clone.process_jobs().await;
        });

        Ok(scheduler)
    }

    async fn create_job(&self, args: &[String]) -> Result<String> {
        if args.len() < 4 {
            return Ok("Usage: schedule create <name> <command> <schedule> [args...]".to_string());
        }

        let name = args[1].clone();
        let command = args[2].clone();
        let schedule_str = args[3].clone();
        let job_args = args[4..].to_vec();

        let schedule = if schedule_str.starts_with("@") {
            match schedule_str.as_str() {
                "@once" => JobSchedule::Once(Utc::now()),
                "@hourly" => JobSchedule::Recurring("0 * * * *".to_string()),
                "@daily" => JobSchedule::Recurring("0 0 * * *".to_string()),
                "@weekly" => JobSchedule::Recurring("0 0 * * 0".to_string()),
                "@monthly" => JobSchedule::Recurring("0 0 1 * *".to_string()),
                "@yearly" => JobSchedule::Recurring("0 0 1 1 *".to_string()),
                _ if schedule_str.starts_with("@every") => {
                    let duration_str = schedule_str.trim_start_matches("@every").trim();
                    let duration = parse_duration(duration_str)?;
                    JobSchedule::Interval(duration)
                }
                _ => return Ok("Invalid schedule format".to_string()),
            }
        } else {
            JobSchedule::Recurring(schedule_str)
        };

        let job = Job::new(
            name,
            command,
            job_args,
            schedule,
            HashMap::new(),
            None,
            None,
            3,
            std::time::Duration::from_secs(30),
            Vec::new(),
        );

        let job_id = self.queue.submit_job(job).await?;
        Ok(format!("Created job with ID: {}", job_id))
    }

    async fn list_jobs(&self, args: &[String]) -> Result<String> {
        let mut filter = JobFilter {
            status: None,
            name: None,
            created_after: None,
            created_before: None,
            command: None,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--status" => {
                    if i + 1 < args.len() {
                        filter.status = match args[i + 1].as_str() {
                            "pending" => Some(JobStatus::Pending),
                            "running" => Some(JobStatus::Running),
                            "completed" => Some(JobStatus::Completed),
                            "cancelled" => Some(JobStatus::Cancelled),
                            "failed" => Some(JobStatus::Failed(String::new())),
                            _ => return Ok("Invalid status filter".to_string()),
                        };
                        i += 2;
                    }
                }
                "--name" => {
                    if i + 1 < args.len() {
                        filter.name = Some(args[i + 1].clone());
                        i += 2;
                    }
                }
                "--after" => {
                    if i + 1 < args.len() {
                        filter.created_after = Some(DateTime::from_str(&args[i + 1])?);
                        i += 2;
                    }
                }
                "--before" => {
                    if i + 1 < args.len() {
                        filter.created_before = Some(DateTime::from_str(&args[i + 1])?);
                        i += 2;
                    }
                }
                "--command" => {
                    if i + 1 < args.len() {
                        filter.command = Some(args[i + 1].clone());
                        i += 2;
                    }
                }
                _ => i += 1,
            }
        }

        let jobs = self.queue.list_jobs(Some(filter)).await;
        if jobs.is_empty() {
            return Ok("No jobs found".to_string());
        }

        let mut output = String::new();
        output.push_str(&format!("{:<36} {:<20} {:<15} {:<20} {:<20}\n",
            "ID", "NAME", "STATUS", "NEXT RUN", "LAST RUN"));

        for job in jobs {
            let status = match &job.status {
                JobStatus::Pending => "PENDING".yellow(),
                JobStatus::Running => "RUNNING".blue(),
                JobStatus::Completed => "COMPLETED".green(),
                JobStatus::Failed(err) => format!("FAILED: {}", err).red(),
                JobStatus::Cancelled => "CANCELLED".red(),
            };

            let next_run = job.metadata.next_run
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "N/A".to_string());

            let last_run = job.metadata.last_run
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "Never".to_string());

            output.push_str(&format!("{:<36} {:<20} {:<15} {:<20} {:<20}\n",
                job.id, job.name, status, next_run, last_run));
        }

        Ok(output)
    }

    async fn cancel_job(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: schedule cancel <job_id>".to_string());
        }

        let job_id = &args[1];
        self.queue.cancel_job(job_id).await?;
        Ok(format!("Cancelled job {}", job_id))
    }

    async fn show_job(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: schedule show <job_id>".to_string());
        }

        let job_id = &args[1];
        if let Some(job) = self.queue.get_job(job_id).await {
            let mut output = String::new();
            output.push_str(&format!("Job Details for {}\n", job_id.bright_green()));
            output.push_str(&format!("Name: {}\n", job.name));
            output.push_str(&format!("Command: {} {}\n", job.command, job.args.join(" ")));
            output.push_str(&format!("Status: {}\n", match &job.status {
                JobStatus::Pending => "Pending".yellow(),
                JobStatus::Running => "Running".blue(),
                JobStatus::Completed => "Completed".green(),
                JobStatus::Failed(err) => format!("Failed: {}", err).red(),
                JobStatus::Cancelled => "Cancelled".red(),
            }));

            output.push_str(&format!("Schedule: {}\n", match &job.schedule {
                JobSchedule::Once(time) => format!("Once at {}", time),
                JobSchedule::Recurring(cron) => format!("Recurring ({})", cron),
                JobSchedule::Interval(duration) => format!("Every {:?}", duration),
            }));

            output.push_str("\nMetadata:\n");
            output.push_str(&format!("  Created: {}\n", job.metadata.created_at));
            output.push_str(&format!("  Updated: {}\n", job.metadata.updated_at));
            output.push_str(&format!("  Last Run: {}\n", 
                job.metadata.last_run.map(|t| t.to_string()).unwrap_or_else(|| "Never".to_string())));
            output.push_str(&format!("  Next Run: {}\n",
                job.metadata.next_run.map(|t| t.to_string()).unwrap_or_else(|| "N/A".to_string())));
            output.push_str(&format!("  Run Count: {}\n", job.metadata.run_count));

            if let Some(result) = self.queue.get_job_result(job_id).await {
                output.push_str("\nLast Result:\n");
                output.push_str(&format!("  Success: {}\n", result.success));
                output.push_str(&format!("  Exit Code: {}\n", 
                    result.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string())));
                output.push_str(&format!("  Completed At: {}\n", result.completed_at));
                if let Some(error) = result.error {
                    output.push_str(&format!("  Error: {}\n", error));
                }
                output.push_str("  Output:\n");
                for line in result.output.lines() {
                    output.push_str(&format!("    {}\n", line));
                }
            }

            Ok(output)
        } else {
            Ok(format!("Job {} not found", job_id))
        }
    }

    async fn cleanup_jobs(&self, args: &[String]) -> Result<String> {
        let days = if args.len() > 1 {
            args[1].parse().unwrap_or(30)
        } else {
            30
        };

        let older_than = Utc::now() - Duration::days(days);
        let count = self.queue.cleanup_old_jobs(older_than).await?;
        Ok(format!("Cleaned up {} old jobs", count))
    }
}

#[async_trait]
impl Plugin for SchedulerPlugin {
    fn name(&self) -> &str {
        "schedule"
    }

    fn description(&self) -> &str {
        "Job scheduling and task management"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("create") => self.create_job(&command.args).await,
            Some("list") => self.list_jobs(&command.args).await,
            Some("cancel") => self.cancel_job(&command.args).await,
            Some("show") => self.show_job(&command.args).await,
            Some("cleanup") => self.cleanup_jobs(&command.args).await,
            _ => Ok("Available commands: create, list, cancel, show, cleanup".to_string()),
        }
    }
}

fn parse_duration(duration_str: &str) -> Result<Duration> {
    let mut total_seconds = 0i64;
    let mut current_number = String::new();

    for c in duration_str.chars() {
        if c.is_digit(10) {
            current_number.push(c);
        } else {
            let number = current_number.parse::<i64>().unwrap_or(0);
            current_number.clear();

            match c {
                's' => total_seconds += number,
                'm' => total_seconds += number * 60,
                'h' => total_seconds += number * 3600,
                'd' => total_seconds += number * 86400,
                'w' => total_seconds += number * 604800,
                _ => return Err(anyhow::anyhow!("Invalid duration format")),
            }
        }
    }

    Ok(Duration::seconds(total_seconds))
}
