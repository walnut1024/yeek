use crate::app::state::AppState;
use crate::http::emitter::SseEventEmitter;
use std::sync::Arc;

#[derive(Clone)]
pub struct HttpRuntimeState {
    pub app_state: Arc<AppState>,
    pub sse: Arc<SseEventEmitter>,
}
