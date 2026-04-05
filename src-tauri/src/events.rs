use tauri::{AppHandle, Emitter};

use crate::types::MessageInfo;

pub fn emit_message_new(app: &AppHandle, message: &MessageInfo) -> Result<(), String> {
    app.emit("message:new", message)
        .map_err(|e| format!("failed to emit message:new: {e}"))
}
