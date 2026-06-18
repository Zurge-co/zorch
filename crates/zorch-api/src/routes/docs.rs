use axum::{routing::get, Json, Router};
use serde_json::json;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api-docs", get(docs_handler))
}

async fn docs_handler() -> Json<serde_json::Value> {
    Json(json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Zorch AI Gateway API",
            "version": "1.0.0",
            "description": "AI Key Orchestration Platform"
        },
        "paths": {
            "/health": {
                "get": {
                    "summary": "Health check",
                    "responses": {
                        "200": {
                            "description": "Service is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "status": { "type": "string", "example": "ok" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/v1/chat/completions": {
                "post": {
                    "summary": "Chat completions",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "model": { "type": "string" },
                                        "messages": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "role": { "type": "string" },
                                                    "content": { "type": "string" }
                                                }
                                            }
                                        },
                                        "stream": { "type": "boolean" },
                                        "temperature": { "type": "number" },
                                        "max_tokens": { "type": "integer" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Successful completion",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "model": { "type": "string" },
                                            "choices": { "type": "array" },
                                            "usage": {
                                                "type": "object",
                                                "properties": {
                                                    "prompt_tokens": { "type": "integer" },
                                                    "completion_tokens": { "type": "integer" },
                                                    "total_tokens": { "type": "integer" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}
