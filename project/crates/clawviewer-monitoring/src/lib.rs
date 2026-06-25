use prometheus::{Counter, Gauge, Histogram, Registry, Encoder, TextEncoder};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: HealthState,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthState,
    pub message: String,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub result: ActionResult,
    pub details: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    DataAccess,
    Configuration,
    Network,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResult {
    Success,
    Failure(String),
    Denied,
}

pub struct MetricsCollector {
    registry: Registry,
    connections_total: Counter,
    connections_active: Gauge,
    capture_fps: Gauge,
    capture_latency: Histogram,
    webrtc_packets_sent: Counter,
    webrtc_packets_received: Counter,
    errors_total: Counter,
    audit_events_total: Counter,
}

impl MetricsCollector {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        let connections_total = Counter::new(
            "clawviewer_connections_total",
            "Total number of connections"
        )?;
        let connections_active = Gauge::new(
            "clawviewer_connections_active",
            "Currently active connections"
        )?;
        let capture_fps = Gauge::new(
            "clawviewer_capture_fps",
            "Current capture FPS"
        )?;
        let capture_latency = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "clawviewer_capture_latency_ms",
                "Capture latency in milliseconds"
            ).buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0])
        )?;
        let webrtc_packets_sent = Counter::new(
            "clawviewer_webrtc_packets_sent_total",
            "Total WebRTC packets sent"
        )?;
        let webrtc_packets_received = Counter::new(
            "clawviewer_webrtc_packets_received_total",
            "Total WebRTC packets received"
        )?;
        let errors_total = Counter::new(
            "clawviewer_errors_total",
            "Total errors"
        )?;
        let audit_events_total = Counter::new(
            "clawviewer_audit_events_total",
            "Total audit events"
        )?;
        
        registry.register(Box::new(connections_total.clone()))?;
        registry.register(Box::new(connections_active.clone()))?;
        registry.register(Box::new(capture_fps.clone()))?;
        registry.register(Box::new(capture_latency.clone()))?;
        registry.register(Box::new(webrtc_packets_sent.clone()))?;
        registry.register(Box::new(webrtc_packets_received.clone()))?;
        registry.register(Box::new(errors_total.clone()))?;
        registry.register(Box::new(audit_events_total.clone()))?;
        
        Ok(Self {
            registry,
            connections_total,
            connections_active,
            capture_fps,
            capture_latency,
            webrtc_packets_sent,
            webrtc_packets_received,
            errors_total,
            audit_events_total,
        })
    }
    
    pub fn record_connection(&self) {
        self.connections_total.inc();
        self.connections_active.inc();
    }
    
    pub fn record_disconnection(&self) {
        self.connections_active.dec();
    }
    
    pub fn record_capture_frame(&self, latency_ms: f64) {
        self.capture_latency.observe(latency_ms);
    }
    
    pub fn set_capture_fps(&self, fps: f64) {
        self.capture_fps.set(fps);
    }
    
    pub fn record_webrtc_packet_sent(&self) {
        self.webrtc_packets_sent.inc();
    }
    
    pub fn record_webrtc_packet_received(&self) {
        self.webrtc_packets_received.inc();
    }
    
    pub fn record_error(&self) {
        self.errors_total.inc();
    }
    
    pub fn record_audit_event(&self) {
        self.audit_events_total.inc();
    }
    
    pub fn render_metrics(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }
}

pub struct AuditLogger {
    events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub async fn log(&self, event: AuditEvent) {
        let mut events = self.events.write().await;
        events.push(event);
        
        if events.len() > 10000 {
            events.remove(0);
        }
    }
    
    pub async fn get_events(
        &self,
        event_type: Option<AuditEventType>,
        limit: usize,
    ) -> Vec<AuditEvent> {
        let events = self.events.read().await;
        let filtered: Vec<_> = events
            .iter()
            .filter(|e| event_type.as_ref().map(|t| std::mem::discriminant(t) == std::mem::discriminant(&e.event_type)).unwrap_or(true))
            .cloned()
            .collect();
        filtered.into_iter().rev().take(limit).collect()
    }
}

pub struct HealthChecker {
    checks: Vec<Box<dyn Fn() -> HealthCheck + Send + Sync>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }
    
    pub fn add_check<F>(&mut self, check: F)
    where
        F: Fn() -> HealthCheck + Send + Sync + 'static,
    {
        self.checks.push(Box::new(check));
    }
    
    pub fn check(&self) -> HealthStatus {
        let checks: Vec<_> = self.checks.iter().map(|c| c()).collect();
        
        let status = if checks.iter().any(|c| matches!(c.status, HealthState::Unhealthy)) {
            HealthState::Unhealthy
        } else if checks.iter().any(|c| matches!(c.status, HealthState::Degraded)) {
            HealthState::Degraded
        } else {
            HealthState::Healthy
        };
        
        HealthStatus {
            status,
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            checks,
        }
    }
}
