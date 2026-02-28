//! Infrastructure manifest extraction API.
//!
//! Provides flattened, generator-friendly representations of infrastructure
//! configuration extracted from parsed SurfDoc manifest blocks.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{Block, CrateEntry, DomainEntry, EnvEntry, SmokeCheck, SurfDoc, VolumeEntry};

/// Flattened app manifest extracted from `::app` and its children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub name: String,
    pub binary: Option<String>,
    pub region: Option<String>,
    pub port: Option<u32>,
    pub platform: Option<String>,
    pub build: Option<BuildConfig>,
    pub database: Option<DatabaseConfig>,
    pub deploys: Vec<DeployConfig>,
    pub env_groups: Vec<EnvGroup>,
    pub health: Option<HealthConfig>,
    pub concurrency: Option<ConcurrencyConfig>,
    pub cicd: Option<CicdConfig>,
    pub smoke_checks: Vec<SmokeCheck>,
    pub domains: Vec<DomainEntry>,
    pub crates: Vec<CrateEntry>,
    pub deploy_urls: Vec<(String, String)>,
    pub volumes: Vec<VolumeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub base: Option<String>,
    pub runtime: Option<String>,
    pub edition: Option<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub name: Option<String>,
    pub shared_auth: bool,
    pub volume_gb: Option<u32>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub env: String,
    pub app: Option<String>,
    pub machines: Option<u32>,
    pub memory: Option<u32>,
    pub auto_stop: Option<String>,
    pub min_machines: Option<u32>,
    pub strategy: Option<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvGroup {
    pub tier: String,
    pub entries: Vec<EnvEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    pub path: String,
    pub method: Option<String>,
    pub grace: Option<String>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    pub concurrency_type: Option<String>,
    pub hard_limit: Option<u32>,
    pub soft_limit: Option<u32>,
    pub force_https: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CicdConfig {
    pub provider: Option<String>,
    pub properties: HashMap<String, String>,
}

/// Extract the first app manifest from a document.
pub fn extract_manifest(doc: &SurfDoc) -> Option<AppManifest> {
    extract_all_manifests(doc).into_iter().next()
}

/// Extract all app manifests from a document.
pub fn extract_all_manifests(doc: &SurfDoc) -> Vec<AppManifest> {
    let mut manifests = Vec::new();
    for block in &doc.blocks {
        if let Block::App {
            name, binary, region, port, platform, children, ..
        } = block
        {
            let mut manifest = AppManifest {
                name: name.clone(),
                binary: binary.clone(),
                region: region.clone(),
                port: *port,
                platform: platform.clone(),
                build: None,
                database: None,
                deploys: Vec::new(),
                env_groups: Vec::new(),
                health: None,
                concurrency: None,
                cicd: None,
                smoke_checks: Vec::new(),
                domains: Vec::new(),
                crates: Vec::new(),
                deploy_urls: Vec::new(),
                volumes: Vec::new(),
            };

            for child in children {
                match child {
                    Block::Build { base, runtime, edition, properties, .. } => {
                        manifest.build = Some(BuildConfig {
                            base: base.clone(),
                            runtime: runtime.clone(),
                            edition: edition.clone(),
                            properties: properties.iter().map(|p| (p.key.clone(), p.value.clone())).collect(),
                        });
                    }
                    Block::InfraDatabase { name, shared_auth, volume_gb, properties, .. } => {
                        manifest.database = Some(DatabaseConfig {
                            name: name.clone(),
                            shared_auth: *shared_auth,
                            volume_gb: *volume_gb,
                            properties: properties.iter().map(|p| (p.key.clone(), p.value.clone())).collect(),
                        });
                    }
                    Block::Deploy { env, app, machines, memory, auto_stop, min_machines, strategy, properties, .. } => {
                        if let Some(env) = env {
                            manifest.deploys.push(DeployConfig {
                                env: env.clone(),
                                app: app.clone(),
                                machines: *machines,
                                memory: *memory,
                                auto_stop: auto_stop.clone(),
                                min_machines: *min_machines,
                                strategy: strategy.clone(),
                                properties: properties.iter().map(|p| (p.key.clone(), p.value.clone())).collect(),
                            });
                        }
                    }
                    Block::InfraEnv { tier, entries, .. } => {
                        manifest.env_groups.push(EnvGroup {
                            tier: tier.clone().unwrap_or_default(),
                            entries: entries.clone(),
                        });
                    }
                    Block::Health { path, method, grace, interval, timeout, .. } => {
                        if let Some(path) = path {
                            manifest.health = Some(HealthConfig {
                                path: path.clone(),
                                method: method.clone(),
                                grace: grace.clone(),
                                interval: interval.clone(),
                                timeout: timeout.clone(),
                            });
                        }
                    }
                    Block::Concurrency { concurrency_type, hard_limit, soft_limit, force_https, .. } => {
                        manifest.concurrency = Some(ConcurrencyConfig {
                            concurrency_type: concurrency_type.clone(),
                            hard_limit: *hard_limit,
                            soft_limit: *soft_limit,
                            force_https: *force_https,
                        });
                    }
                    Block::Cicd { provider, properties, .. } => {
                        manifest.cicd = Some(CicdConfig {
                            provider: provider.clone(),
                            properties: properties.iter().map(|p| (p.key.clone(), p.value.clone())).collect(),
                        });
                    }
                    Block::Smoke { checks, .. } => {
                        manifest.smoke_checks.extend(checks.clone());
                    }
                    Block::Domains { entries, .. } => {
                        manifest.domains.extend(entries.clone());
                    }
                    Block::Crates { entries, .. } => {
                        manifest.crates.extend(entries.clone());
                    }
                    Block::DeployUrls { entries, .. } => {
                        manifest.deploy_urls.extend(entries.iter().map(|p| (p.key.clone(), p.value.clone())));
                    }
                    Block::Volumes { entries, .. } => {
                        manifest.volumes.extend(entries.clone());
                    }
                    _ => {}
                }
            }

            manifests.push(manifest);
        }
    }
    manifests
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn extract_manifest_from_empty_doc() {
        let result = parse("# Hello\n");
        assert!(extract_manifest(&result.doc).is_none());
    }

    #[test]
    fn extract_manifest_basic() {
        let src = r#"---
title: test
type: manifest
---

::app[name=test-app binary=test-server region=sjc port=8080]

:::deploy[env=develop app=test-app-develop machines=1 memory=256]
:::

:::deploy[env=production app=test-app machines=2 memory=512]
:::

:::health[path=/healthz method=GET]
:::

:::env[tier=required]
DATABASE_URL
SESSION_SECRET
:::

:::env[tier=defaults]
HOST = 0.0.0.0
PORT = 8080
:::

:::smoke[script=scripts/smoke-test.sh]
GET /healthz -> 200
GET /login -> 200
POST /api/test -> 201
:::

:::domains
example.com (main site)
api.example.com
:::

:::database[name=test_db shared_auth volume_gb=1]
migrations: 64
:::

:::concurrency[type=requests hard_limit=250 soft_limit=200 force_https]
:::

:::build[base=rust:1.89 runtime=debian:bookworm-slim edition=2024]
cache: true
:::

:::cicd[provider=github-actions]
tests: cargo test --workspace
:::

:::crates
my-crate (github: org/repo, branch: main)
other-crate (features: pdf)
:::

:::deploy_urls
develop: https://test-develop.fly.dev
production: https://test.fly.dev
:::

:::volumes
data -> /app/data
uploads -> /app/uploads
:::

::
"#;
        let result = parse(src);
        let manifest = extract_manifest(&result.doc).expect("should extract manifest");
        assert_eq!(manifest.name, "test-app");
        assert_eq!(manifest.binary.as_deref(), Some("test-server"));
        assert_eq!(manifest.region.as_deref(), Some("sjc"));
        assert_eq!(manifest.port, Some(8080));

        assert_eq!(manifest.deploys.len(), 2);
        assert_eq!(manifest.deploys[0].env, "develop");
        assert_eq!(manifest.deploys[1].env, "production");
        assert_eq!(manifest.deploys[1].machines, Some(2));

        assert!(manifest.health.is_some());
        assert_eq!(manifest.health.as_ref().unwrap().path, "/healthz");

        assert_eq!(manifest.env_groups.len(), 2);
        assert_eq!(manifest.env_groups[0].tier, "required");
        assert_eq!(manifest.env_groups[0].entries.len(), 2);
        assert_eq!(manifest.env_groups[1].tier, "defaults");
        assert_eq!(manifest.env_groups[1].entries[0].name, "HOST");
        assert_eq!(manifest.env_groups[1].entries[0].default_value.as_deref(), Some("0.0.0.0"));

        assert_eq!(manifest.smoke_checks.len(), 3);
        assert_eq!(manifest.smoke_checks[0].method, "GET");
        assert_eq!(manifest.smoke_checks[0].path, "/healthz");
        assert_eq!(manifest.smoke_checks[0].expected, 200);
        assert_eq!(manifest.smoke_checks[2].method, "POST");
        assert_eq!(manifest.smoke_checks[2].expected, 201);

        assert_eq!(manifest.domains.len(), 2);
        assert_eq!(manifest.domains[0].domain, "example.com");
        assert_eq!(manifest.domains[0].description.as_deref(), Some("main site"));
        assert!(manifest.domains[1].description.is_none());

        assert!(manifest.database.is_some());
        let db = manifest.database.as_ref().unwrap();
        assert_eq!(db.name.as_deref(), Some("test_db"));
        assert!(db.shared_auth);
        assert_eq!(db.volume_gb, Some(1));

        assert!(manifest.concurrency.is_some());
        let conc = manifest.concurrency.as_ref().unwrap();
        assert_eq!(conc.hard_limit, Some(250));
        assert_eq!(conc.soft_limit, Some(200));
        assert!(conc.force_https);

        assert!(manifest.build.is_some());
        let build = manifest.build.as_ref().unwrap();
        assert_eq!(build.base.as_deref(), Some("rust:1.89"));

        assert!(manifest.cicd.is_some());
        assert_eq!(manifest.cicd.as_ref().unwrap().provider.as_deref(), Some("github-actions"));

        assert_eq!(manifest.crates.len(), 2);
        assert_eq!(manifest.crates[0].name, "my-crate");

        assert_eq!(manifest.deploy_urls.len(), 2);
        assert_eq!(manifest.deploy_urls[0].0, "develop");

        assert_eq!(manifest.volumes.len(), 2);
        assert_eq!(manifest.volumes[0].name, "data");
        assert_eq!(manifest.volumes[0].mount, "/app/data");
    }

    #[test]
    fn extract_no_manifest() {
        let result = parse("# Hello\n\n::callout[type=info]\nHi\n::\n");
        assert!(extract_manifest(&result.doc).is_none());
    }

    #[test]
    fn extract_empty_app() {
        let src = "::app[name=empty]\n::\n";
        let result = parse(src);
        let manifest = extract_manifest(&result.doc).expect("should extract");
        assert_eq!(manifest.name, "empty");
        assert!(manifest.build.is_none());
        assert!(manifest.deploys.is_empty());
        assert!(manifest.health.is_none());
    }

    #[test]
    fn extract_multiple_manifests() {
        let src = r#"::app[name=app1]
:::deploy[env=production]
:::
::

::app[name=app2]
:::deploy[env=staging]
:::
::
"#;
        let result = parse(src);
        let manifests = extract_all_manifests(&result.doc);
        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0].name, "app1");
        assert_eq!(manifests[0].deploys[0].env, "production");
        assert_eq!(manifests[1].name, "app2");
        assert_eq!(manifests[1].deploys[0].env, "staging");
    }
}
