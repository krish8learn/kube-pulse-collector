use chrono::{SecondsFormat, Utc};
use tonic::{Request, Response, Status};
use tracing::{info, instrument};

use crate::proto::health::v1::{
    health_service_server::HealthService, HealthCheckRequest, HealthCheckResponse,
};

pub struct HealthServiceImpl;

#[tonic::async_trait]
impl HealthService for HealthServiceImpl {
    #[instrument(skip(self, _request), fields(caller = %_request.get_ref().service))]
    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let reply = HealthCheckResponse {
            status: "ok".into(),
            service: "kube-pulse-collector".into(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        };

        info!(status = %reply.status, "health check ok");

        Ok(Response::new(reply))
    }
}
