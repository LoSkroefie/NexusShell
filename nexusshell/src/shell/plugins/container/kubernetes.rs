use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    Client,
    config::{KubeConfigOptions, Kubeconfig},
    core::ObjectMeta,
};
use k8s_openapi::api::{
    core::v1::{Pod, Service, ConfigMap, Secret},
    apps::v1::{Deployment, StatefulSet},
};
use futures::StreamExt;
use anyhow::{Result, Context};
use tokio::fs;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::BTreeMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
struct KubernetesConfig {
    context: String,
    namespace: String,
    kubeconfig_path: PathBuf,
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        let mut kubeconfig_path = dirs::home_dir().unwrap_or_default();
        kubeconfig_path.push(".kube");
        kubeconfig_path.push("config");

        KubernetesConfig {
            context: "default".to_string(),
            namespace: "default".to_string(),
            kubeconfig_path,
        }
    }
}

pub struct KubernetesPlugin {
    config: KubernetesConfig,
    client: Option<Client>,
}

impl KubernetesPlugin {
    pub async fn new() -> Result<Self> {
        let config = Self::load_config().await.unwrap_or_default();
        Ok(KubernetesPlugin {
            config,
            client: None,
        })
    }

    async fn load_config() -> Result<KubernetesConfig> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("kubernetes_config.json");

        if !config_path.exists() {
            let config = KubernetesConfig::default();
            fs::create_dir_all(config_path.parent().unwrap()).await?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
            Ok(config)
        } else {
            let content = fs::read_to_string(&config_path).await?;
            Ok(serde_json::from_str(&content)?)
        }
    }

    async fn init_client(&mut self) -> Result<()> {
        if self.client.is_none() {
            let kubeconfig = Kubeconfig::read_from(&self.config.kubeconfig_path)?;
            let options = KubeConfigOptions {
                context: Some(self.config.context.clone()),
                ..Default::default()
            };
            let config = kube::Config::from_custom_kubeconfig(kubeconfig, &options).await?;
            self.client = Some(Client::try_from(config)?);
        }
        Ok(())
    }

    async fn list_pods(&self) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let pods: Api<Pod> = Api::namespaced(client.clone(), &self.config.namespace);
        let pod_list = pods.list(&ListParams::default()).await?;

        let mut output = String::new();
        output.push_str(&format!("{}\n", "PODS".bright_green()));
        output.push_str(&format!("{:<30} {:<15} {:<10} {:<15} {:<20}\n",
            "NAME", "STATUS", "RESTARTS", "AGE", "IP"));

        for pod in pod_list.items {
            let name = pod.metadata.name.unwrap_or_default();
            let status = pod.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_default();
            let restarts = pod.status.as_ref()
                .and_then(|s| s.container_statuses.as_ref())
                .and_then(|cs| cs.first())
                .map(|c| c.restart_count)
                .unwrap_or(0);
            let age = pod.metadata.creation_timestamp
                .map(|t| humantime::format_duration(Utc::now().signed_duration_since(t).to_std().unwrap_or_default()).to_string())
                .unwrap_or_default();
            let ip = pod.status.as_ref()
                .and_then(|s| s.pod_ip.clone())
                .unwrap_or_default();

            output.push_str(&format!("{:<30} {:<15} {:<10} {:<15} {:<20}\n",
                name, status, restarts, age, ip));
        }

        Ok(output)
    }

    async fn list_deployments(&self) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let deployments: Api<Deployment> = Api::namespaced(client.clone(), &self.config.namespace);
        let deployment_list = deployments.list(&ListParams::default()).await?;

        let mut output = String::new();
        output.push_str(&format!("{}\n", "DEPLOYMENTS".bright_green()));
        output.push_str(&format!("{:<30} {:<10} {:<10} {:<10} {:<15}\n",
            "NAME", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"));

        for deployment in deployment_list.items {
            let name = deployment.metadata.name.unwrap_or_default();
            let status = deployment.status.as_ref().unwrap();
            let ready = format!("{}/{}", 
                status.ready_replicas.unwrap_or(0),
                status.replicas.unwrap_or(0));
            let up_to_date = status.updated_replicas.unwrap_or(0);
            let available = status.available_replicas.unwrap_or(0);
            let age = deployment.metadata.creation_timestamp
                .map(|t| humantime::format_duration(Utc::now().signed_duration_since(t).to_std().unwrap_or_default()).to_string())
                .unwrap_or_default();

            output.push_str(&format!("{:<30} {:<10} {:<10} {:<10} {:<15}\n",
                name, ready, up_to_date, available, age));
        }

        Ok(output)
    }

    async fn list_services(&self) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let services: Api<Service> = Api::namespaced(client.clone(), &self.config.namespace);
        let service_list = services.list(&ListParams::default()).await?;

        let mut output = String::new();
        output.push_str(&format!("{}\n", "SERVICES".bright_green()));
        output.push_str(&format!("{:<30} {:<15} {:<20} {:<15} {:<20}\n",
            "NAME", "TYPE", "CLUSTER-IP", "EXTERNAL-IP", "PORTS"));

        for service in service_list.items {
            let name = service.metadata.name.unwrap_or_default();
            let service_type = service.spec.as_ref()
                .and_then(|s| s.type_.clone())
                .unwrap_or_default();
            let cluster_ip = service.spec.as_ref()
                .and_then(|s| s.cluster_ip.clone())
                .unwrap_or_default();
            let external_ip = service.status.as_ref()
                .and_then(|s| s.load_balancer.as_ref())
                .and_then(|lb| lb.ingress.as_ref())
                .and_then(|i| i.first())
                .and_then(|i| i.ip.clone())
                .unwrap_or_default();
            let ports = service.spec.as_ref()
                .map(|s| s.ports.as_ref())
                .unwrap_or(None)
                .map(|ports| ports.iter()
                    .map(|p| format!("{}:{}", p.port, p.target_port.as_ref().map_or(0, |t| t.as_u16().unwrap_or(0))))
                    .collect::<Vec<_>>()
                    .join(", "))
                .unwrap_or_default();

            output.push_str(&format!("{:<30} {:<15} {:<20} {:<15} {:<20}\n",
                name, service_type, cluster_ip, external_ip, ports));
        }

        Ok(output)
    }

    async fn get_pod_logs(&self, pod_name: &str) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let pods: Api<Pod> = Api::namespaced(client.clone(), &self.config.namespace);
        
        let mut params = BTreeMap::new();
        params.insert("tailLines", "100");
        params.insert("timestamps", "true");

        let logs = pods.logs(pod_name, &params).await?;
        Ok(logs)
    }

    async fn delete_resource(&self, resource_type: &str, name: &str) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();

        match resource_type {
            "pod" => {
                let pods: Api<Pod> = Api::namespaced(client.clone(), &self.config.namespace);
                pods.delete(name, &DeleteParams::default()).await?;
            }
            "deployment" => {
                let deployments: Api<Deployment> = Api::namespaced(client.clone(), &self.config.namespace);
                deployments.delete(name, &DeleteParams::default()).await?;
            }
            "service" => {
                let services: Api<Service> = Api::namespaced(client.clone(), &self.config.namespace);
                services.delete(name, &DeleteParams::default()).await?;
            }
            _ => return Err(anyhow::anyhow!("Unsupported resource type")),
        }

        Ok(format!("Deleted {} {}", resource_type, name))
    }

    async fn scale_deployment(&self, name: &str, replicas: i32) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let deployments: Api<Deployment> = Api::namespaced(client.clone(), &self.config.namespace);
        
        let deployment = deployments.get(name).await?;
        let mut deployment_patch = deployment.clone();
        deployment_patch.spec.as_mut().unwrap().replicas = Some(replicas);

        deployments.replace(name, &PostParams::default(), &deployment_patch).await?;
        Ok(format!("Scaled deployment {} to {} replicas", name, replicas))
    }

    async fn describe_pod(&self, name: &str) -> Result<String> {
        self.init_client().await?;
        let client = self.client.as_ref().unwrap();
        let pods: Api<Pod> = Api::namespaced(client.clone(), &self.config.namespace);
        
        let pod = pods.get(name).await?;
        let mut output = String::new();

        output.push_str(&format!("Pod Description: {}\n", name.bright_green()));
        output.push_str("Metadata:\n");
        output.push_str(&format!("  Namespace: {}\n", pod.metadata.namespace.unwrap_or_default()));
        output.push_str(&format!("  Creation Time: {}\n", 
            pod.metadata.creation_timestamp.map(|t| t.to_rfc3339()).unwrap_or_default()));
        
        if let Some(status) = pod.status {
            output.push_str("\nStatus:\n");
            output.push_str(&format!("  Phase: {}\n", status.phase.unwrap_or_default()));
            output.push_str(&format!("  Pod IP: {}\n", status.pod_ip.unwrap_or_default()));
            output.push_str(&format!("  Host IP: {}\n", status.host_ip.unwrap_or_default()));
            
            if let Some(conditions) = status.conditions {
                output.push_str("\nConditions:\n");
                for condition in conditions {
                    output.push_str(&format!("  Type: {}\n", condition.type_));
                    output.push_str(&format!("  Status: {}\n", condition.status));
                    if let Some(message) = condition.message {
                        output.push_str(&format!("  Message: {}\n", message));
                    }
                }
            }
        }

        if let Some(spec) = pod.spec {
            output.push_str("\nSpec:\n");
            output.push_str(&format!("  Node Name: {}\n", spec.node_name.unwrap_or_default()));
            
            if let Some(containers) = spec.containers {
                output.push_str("\nContainers:\n");
                for container in containers {
                    output.push_str(&format!("  - Name: {}\n", container.name));
                    output.push_str(&format!("    Image: {}\n", container.image.unwrap_or_default()));
                    if let Some(ports) = container.ports {
                        output.push_str("    Ports:\n");
                        for port in ports {
                            output.push_str(&format!("      - {}/{}\n", 
                                port.container_port,
                                port.protocol.unwrap_or_default()));
                        }
                    }
                }
            }
        }

        Ok(output)
    }
}

#[async_trait]
impl Plugin for KubernetesPlugin {
    fn name(&self) -> &str {
        "kubectl"
    }

    fn description(&self) -> &str {
        "Kubernetes cluster management and operations"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("get") => {
                if command.args.len() < 2 {
                    return Ok("Usage: kubectl get [pods|deployments|services]".to_string());
                }
                match command.args[1].as_str() {
                    "pods" => self.list_pods().await,
                    "deployments" => self.list_deployments().await,
                    "services" => self.list_services().await,
                    _ => Ok("Supported resources: pods, deployments, services".to_string()),
                }
            }

            Some("logs") => {
                if command.args.len() < 2 {
                    return Ok("Usage: kubectl logs <pod_name>".to_string());
                }
                self.get_pod_logs(&command.args[1]).await
            }

            Some("delete") => {
                if command.args.len() < 3 {
                    return Ok("Usage: kubectl delete <resource_type> <name>".to_string());
                }
                self.delete_resource(&command.args[1], &command.args[2]).await
            }

            Some("scale") => {
                if command.args.len() < 4 {
                    return Ok("Usage: kubectl scale deployment <name> --replicas=<count>".to_string());
                }
                let replicas = command.args[3]
                    .strip_prefix("--replicas=")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| anyhow::anyhow!("Invalid replicas format"))?;
                self.scale_deployment(&command.args[2], replicas).await
            }

            Some("describe") => {
                if command.args.len() < 3 {
                    return Ok("Usage: kubectl describe pod <name>".to_string());
                }
                match command.args[1].as_str() {
                    "pod" => self.describe_pod(&command.args[2]).await,
                    _ => Ok("Currently only pod descriptions are supported".to_string()),
                }
            }

            Some("config") => {
                if command.args.len() < 3 {
                    return Ok("Usage: kubectl config [use-context|set-namespace] <value>".to_string());
                }
                match command.args[1].as_str() {
                    "use-context" => {
                        self.config.context = command.args[2].clone();
                        self.client = None; // Force client reinitialization
                        Ok(format!("Switched to context {}", command.args[2]))
                    }
                    "set-namespace" => {
                        self.config.namespace = command.args[2].clone();
                        Ok(format!("Switched to namespace {}", command.args[2]))
                    }
                    _ => Ok("Supported config commands: use-context, set-namespace".to_string()),
                }
            }

            _ => Ok("Available commands: get, logs, delete, scale, describe, config".to_string()),
        }
    }
}
