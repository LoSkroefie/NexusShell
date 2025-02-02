use super::job::{Job, JobResult, JobStatus, JobFilter};
use tokio::sync::{mpsc, RwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use anyhow::Result;
use tokio::time::{self, Duration};
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tokio::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueConfig {
    pub max_concurrent_jobs: usize,
    pub max_retries: u32,
    pub default_timeout: Duration,
    pub storage_path: PathBuf,
}

impl Default for QueueConfig {
    fn default() -> Self {
        QueueConfig {
            max_concurrent_jobs: 10,
            max_retries: 3,
            default_timeout: Duration::from_secs(3600),
            storage_path: PathBuf::from(".nexusshell/jobs"),
        }
    }
}

#[derive(Debug)]
pub struct JobQueue {
    jobs: Arc<RwLock<HashMap<String, Job>>>,
    pending: Arc<RwLock<VecDeque<String>>>,
    running: Arc<RwLock<HashSet<String>>>,
    completed: Arc<RwLock<Vec<JobResult>>>,
    config: QueueConfig,
    tx: mpsc::Sender<JobResult>,
    rx: Arc<RwLock<mpsc::Receiver<JobResult>>>,
}

impl JobQueue {
    pub async fn new(config: QueueConfig) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let queue = JobQueue {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(VecDeque::new())),
            running: Arc::new(RwLock::new(HashSet::new())),
            completed: Arc::new(RwLock::new(Vec::new())),
            config,
            tx,
            rx: Arc::new(RwLock::new(rx)),
        };

        queue.load_state().await?;
        Ok(queue)
    }

    async fn load_state(&self) -> Result<()> {
        if !self.config.storage_path.exists() {
            fs::create_dir_all(&self.config.storage_path).await?;
            return Ok(());
        }

        let mut jobs = self.jobs.write().await;
        let mut pending = self.pending.write().await;
        let mut completed = self.completed.write().await;

        let jobs_path = self.config.storage_path.join("jobs.json");
        if jobs_path.exists() {
            let content = fs::read_to_string(&jobs_path).await?;
            let stored_jobs: HashMap<String, Job> = serde_json::from_str(&content)?;
            *jobs = stored_jobs;
        }

        let pending_path = self.config.storage_path.join("pending.json");
        if pending_path.exists() {
            let content = fs::read_to_string(&pending_path).await?;
            let stored_pending: VecDeque<String> = serde_json::from_str(&content)?;
            *pending = stored_pending;
        }

        let completed_path = self.config.storage_path.join("completed.json");
        if completed_path.exists() {
            let content = fs::read_to_string(&completed_path).await?;
            let stored_completed: Vec<JobResult> = serde_json::from_str(&content)?;
            *completed = stored_completed;
        }

        Ok(())
    }

    async fn save_state(&self) -> Result<()> {
        let jobs = self.jobs.read().await;
        let pending = self.pending.read().await;
        let completed = self.completed.read().await;

        fs::create_dir_all(&self.config.storage_path).await?;

        let jobs_path = self.config.storage_path.join("jobs.json");
        fs::write(&jobs_path, serde_json::to_string_pretty(&*jobs)?).await?;

        let pending_path = self.config.storage_path.join("pending.json");
        fs::write(&pending_path, serde_json::to_string_pretty(&*pending)?).await?;

        let completed_path = self.config.storage_path.join("completed.json");
        fs::write(&completed_path, serde_json::to_string_pretty(&*completed)?).await?;

        Ok(())
    }

    pub async fn submit_job(&self, job: Job) -> Result<String> {
        let job_id = job.id.clone();
        let mut jobs = self.jobs.write().await;
        let mut pending = self.pending.write().await;

        jobs.insert(job_id.clone(), job);
        pending.push_back(job_id.clone());
        
        self.save_state().await?;
        Ok(job_id)
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        let mut pending = self.pending.write().await;
        let mut running = self.running.write().await;

        if let Some(job) = jobs.get_mut(job_id) {
            job.update_status(JobStatus::Cancelled);
            pending.retain(|id| id != job_id);
            running.remove(job_id);
        }

        self.save_state().await?;
        Ok(())
    }

    pub async fn get_job(&self, job_id: &str) -> Option<Job> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id).cloned()
    }

    pub async fn list_jobs(&self, filter: Option<JobFilter>) -> Vec<Job> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|job| {
                if let Some(filter) = &filter {
                    filter.matches(job)
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    pub async fn get_job_result(&self, job_id: &str) -> Option<JobResult> {
        let completed = self.completed.read().await;
        completed.iter()
            .find(|result| result.job_id == job_id)
            .cloned()
    }

    pub async fn process_jobs(&self) {
        loop {
            self.check_and_start_jobs().await;
            self.process_completed_jobs().await;
            time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn check_and_start_jobs(&self) {
        let running_count = self.running.read().await.len();
        if running_count >= self.config.max_concurrent_jobs {
            return;
        }

        let mut pending = self.pending.write().await;
        let mut running = self.running.write().await;
        let mut jobs = self.jobs.write().await;

        while running.len() < self.config.max_concurrent_jobs {
            if let Some(job_id) = pending.pop_front() {
                if let Some(job) = jobs.get_mut(&job_id) {
                    let now = Utc::now();
                    if let Some(next_run) = job.metadata.next_run {
                        if next_run > now {
                            pending.push_back(job_id);
                            continue;
                        }
                    }

                    let can_run = job.dependencies.iter().all(|dep_id| {
                        if let Some(dep_job) = jobs.get(dep_id) {
                            matches!(dep_job.status, JobStatus::Completed)
                        } else {
                            false
                        }
                    });

                    if !can_run {
                        pending.push_back(job_id);
                        continue;
                    }

                    let tx = self.tx.clone();
                    let mut job_clone = job.clone();
                    tokio::spawn(async move {
                        if let Err(e) = job_clone.execute(tx).await {
                            eprintln!("Job execution error: {}", e);
                        }
                    });

                    running.insert(job_id);
                }
            } else {
                break;
            }
        }

        self.save_state().await.unwrap_or_else(|e| {
            eprintln!("Error saving queue state: {}", e);
        });
    }

    async fn process_completed_jobs(&self) {
        let mut rx = self.rx.write().await;
        while let Ok(result) = rx.try_recv() {
            let mut running = self.running.write().await;
            let mut jobs = self.jobs.write().await;
            let mut completed = self.completed.write().await;
            let mut pending = self.pending.write().await;

            running.remove(&result.job_id);
            if let Some(job) = jobs.get_mut(&result.job_id) {
                match job.schedule {
                    super::job::JobSchedule::Once(_) => {
                        // Job is done, no need to reschedule
                    }
                    super::job::JobSchedule::Recurring(_) | super::job::JobSchedule::Interval(_) => {
                        job.update_next_run();
                        pending.push_back(result.job_id.clone());
                    }
                }
            }

            completed.push(result);
            self.save_state().await.unwrap_or_else(|e| {
                eprintln!("Error saving queue state: {}", e);
            });
        }
    }

    pub async fn cleanup_old_jobs(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let mut jobs = self.jobs.write().await;
        let mut completed = self.completed.write().await;
        let mut count = 0;

        // Remove old completed jobs
        jobs.retain(|_, job| {
            if matches!(job.status, JobStatus::Completed | JobStatus::Failed(_)) {
                if let Some(last_run) = job.metadata.last_run {
                    if last_run < older_than {
                        count += 1;
                        return false;
                    }
                }
            }
            true
        });

        // Remove old job results
        completed.retain(|result| result.completed_at >= older_than);

        self.save_state().await?;
        Ok(count)
    }
}

#[async_trait]
pub trait QueueManager: Send + Sync {
    async fn submit_job(&self, job: Job) -> Result<String>;
    async fn cancel_job(&self, job_id: &str) -> Result<()>;
    async fn get_job(&self, job_id: &str) -> Option<Job>;
    async fn list_jobs(&self, filter: Option<JobFilter>) -> Vec<Job>;
    async fn get_job_result(&self, job_id: &str) -> Option<JobResult>;
}
