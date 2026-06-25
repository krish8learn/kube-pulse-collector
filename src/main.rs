use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod health_service;
mod cluster_service;

mod proto {
    pub mod health {
        pub mod v1 {
            tonic::include_proto!("kubepulse.health.v1");
        }
    }
    pub mod cluster {
        pub mod v1 {
            tonic::include_proto!("kubepulse.cluster.v1");
        }
    }
}

use proto::health::v1::health_service_server::HealthServiceServer;
use proto::cluster::v1::cluster_service_server::ClusterServiceServer;
use health_service::HealthServiceImpl;
use cluster_service::ClusterServiceImpl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer().json())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let addr = "0.0.0.0:50051".parse()?;

    let cluster_svc = ClusterServiceImpl::new().await?;

    info!(addr = %addr, "kube-pulse-collector starting");

    Server::builder()
        .add_service(HealthServiceServer::new(HealthServiceImpl))
        .add_service(ClusterServiceServer::new(cluster_svc))
        .serve(addr)
        .await?;

    Ok(())
}
