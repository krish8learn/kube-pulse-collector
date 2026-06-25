use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{api::ListParams, Api, Client};
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};

use crate::proto::cluster::v1::{
    cluster_service_server::ClusterService, ClusterSummary, GetClusterSummaryRequest,
    GetClusterSummaryResponse, GetNodeRequest, GetNodeResponse, GetNodesRequest, GetNodesResponse,
    NodeInfo,
};

pub struct ClusterServiceImpl {
    client: Client,
}

impl ClusterServiceImpl {
    pub async fn new() -> anyhow::Result<Self> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }
}

#[tonic::async_trait]
impl ClusterService for ClusterServiceImpl {
    #[instrument(skip(self))]
    async fn get_cluster_summary(
        &self,
        _request: Request<GetClusterSummaryRequest>,
    ) -> Result<Response<GetClusterSummaryResponse>, Status> {
        let nodes_api: Api<Node> = Api::all(self.client.clone());
        let pods_api: Api<Pod> = Api::all(self.client.clone());

        let node_list = nodes_api.list(&ListParams::default()).await.map_err(|e| {
            error!(err = %e, "list nodes failed");
            Status::internal(e.to_string())
        })?;

        let pod_list = pods_api.list(&ListParams::default()).await.map_err(|e| {
            error!(err = %e, "list pods failed");
            Status::internal(e.to_string())
        })?;

        let total_nodes = node_list.items.len() as i32;
        let ready_nodes = node_list.items.iter().filter(|n| node_is_ready(n)).count() as i32;
        let total_pods = pod_list.items.len() as i32;
        let running_pods = pod_list
            .items
            .iter()
            .filter(|p| {
                p.status
                    .as_ref()
                    .and_then(|s| s.phase.as_deref())
                    == Some("Running")
            })
            .count() as i32;

        info!(total_nodes, ready_nodes, total_pods, running_pods, "cluster summary ok");

        Ok(Response::new(GetClusterSummaryResponse {
            summary: Some(ClusterSummary {
                total_nodes,
                ready_nodes,
                total_pods,
                running_pods,
            }),
        }))
    }

    #[instrument(skip(self))]
    async fn get_nodes(
        &self,
        _request: Request<GetNodesRequest>,
    ) -> Result<Response<GetNodesResponse>, Status> {
        let nodes_api: Api<Node> = Api::all(self.client.clone());
        let pods_api: Api<Pod> = Api::all(self.client.clone());

        let node_list = nodes_api.list(&ListParams::default()).await.map_err(|e| {
            error!(err = %e, "list nodes failed");
            Status::internal(e.to_string())
        })?;

        let pod_list = pods_api.list(&ListParams::default()).await.map_err(|e| {
            error!(err = %e, "list pods failed");
            Status::internal(e.to_string())
        })?;

        let nodes = node_list
            .items
            .iter()
            .map(|n| node_to_proto(n, &pod_list.items))
            .collect();

        info!(count = node_list.items.len(), "list nodes ok");
        Ok(Response::new(GetNodesResponse { nodes }))
    }

    #[instrument(skip(self), fields(node_name = %request.get_ref().name))]
    async fn get_node(
        &self,
        request: Request<GetNodeRequest>,
    ) -> Result<Response<GetNodeResponse>, Status> {
        let name = &request.get_ref().name;
        let nodes_api: Api<Node> = Api::all(self.client.clone());
        let pods_api: Api<Pod> = Api::all(self.client.clone());

        let node = nodes_api.get(name).await.map_err(|e| match &e {
            kube::Error::Api(api_err) if api_err.code == 404 => {
                Status::not_found(format!("node {:?} not found", name))
            }
            _ => {
                error!(err = %e, node_name = %name, "get node failed");
                Status::internal(e.to_string())
            }
        })?;

        let pod_list = pods_api.list(&ListParams::default()).await.map_err(|e| {
            error!(err = %e, "list pods failed");
            Status::internal(e.to_string())
        })?;

        info!(node_name = %name, "get node ok");
        Ok(Response::new(GetNodeResponse {
            node: Some(node_to_proto(&node, &pod_list.items)),
        }))
    }
}

fn node_is_ready(node: &Node) -> bool {
    node.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True")
        })
        .unwrap_or(false)
}

fn node_to_proto(node: &Node, all_pods: &[Pod]) -> NodeInfo {
    let name = node.metadata.name.clone().unwrap_or_default();

    let pod_count = all_pods
        .iter()
        .filter(|p| {
            p.spec
                .as_ref()
                .and_then(|s| s.node_name.as_deref())
                == Some(name.as_str())
        })
        .count() as i32;

    let capacity = node.status.as_ref().and_then(|s| s.capacity.as_ref());
    let allocatable = node.status.as_ref().and_then(|s| s.allocatable.as_ref());

    let cpu_capacity = capacity
        .and_then(|c| c.get("cpu"))
        .map(|q| q.0.clone())
        .unwrap_or_default();
    let memory_capacity = capacity
        .and_then(|c| c.get("memory"))
        .map(|q| q.0.clone())
        .unwrap_or_default();
    let cpu_allocatable = allocatable
        .and_then(|c| c.get("cpu"))
        .map(|q| q.0.clone())
        .unwrap_or_default();
    let memory_allocatable = allocatable
        .and_then(|c| c.get("memory"))
        .map(|q| q.0.clone())
        .unwrap_or_default();

    let kubernetes_version = node
        .status
        .as_ref()
        .and_then(|s| s.node_info.as_ref())
        .map(|ni| ni.kubelet_version.clone())
        .unwrap_or_default();

    let age = node
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|t| t.0.to_rfc3339())
        .unwrap_or_default();

    let status = if node_is_ready(node) {
        "Ready"
    } else {
        "NotReady"
    }
    .to_string();

    NodeInfo {
        name,
        status,
        cpu_capacity,
        memory_capacity,
        cpu_allocatable,
        memory_allocatable,
        pod_count,
        kubernetes_version,
        age,
    }
}
