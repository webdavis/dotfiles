use posture_application::GatewayHealth;
use std::time::Duration;
pub struct GatewayProbe {
    url: String,
    timeout: Duration,
}
impl GatewayProbe {
    pub fn new(url: String, timeout: Duration) -> Self {
        Self { url, timeout }
    }
}
impl GatewayHealth for GatewayProbe {
    fn status(&mut self) -> Option<u16> {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .new_agent()
            .get(&self.url)
            .call()
            .ok()
            .map(|response| response.status().as_u16())
    }
}
#[cfg(test)]
mod tests;
