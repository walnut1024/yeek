use std::sync::Arc;
use crate::app::state::AppState;
use crate::http::emitter::SseEventEmitter;

#[derive(Clone)]
pub struct HttpRuntimeState {
    pub app_state: Arc<AppState>,
    pub sse: Arc<SseEventEmitter>,
}
