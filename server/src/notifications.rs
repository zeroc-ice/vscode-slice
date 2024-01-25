// Copyright (c) ZeroC, Inc.

use lsp_types::notification::Notification;
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types;

#[derive(Debug)]
pub struct ShowNotification;

impl Notification for ShowNotification {
    type Params = ShowNotificationParams;
    const METHOD: &'static str = "custom/showNotification";
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ShowNotificationParams {
    pub message: String,
    pub message_type: MessageType,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum MessageType {
    Error,
    Warning,
    Info,
}
