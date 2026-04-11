#[cfg(debug_assertions)]
use std::fmt::Display;

#[cfg(debug_assertions)]
pub(crate) fn short_id(value: &str) -> String {
    if value.len() <= 16 {
        value.to_string()
    } else {
        format!("{}…{}", &value[..8], &value[value.len() - 6..])
    }
}

#[cfg(debug_assertions)]
pub(crate) fn error_kind(error: &impl Display) -> &'static str {
    let message = error.to_string().to_lowercase();
    if message.contains("timeout") {
        "timeout"
    } else if message.contains("network")
        || message.contains("connection")
        || message.contains("request")
    {
        "network"
    } else if message.contains("rpc") {
        "rpc"
    } else if message.contains("auth") || message.contains("biometric") {
        "auth"
    } else if message.contains("paymaster") {
        "paymaster"
    } else if message.contains("insufficient") {
        "insufficient_balance"
    } else if message.contains("state") || message.contains("progress") {
        "invalid_state"
    } else {
        "unknown"
    }
}

#[cfg(debug_assertions)]
pub(crate) fn emit(level: &str, target: &str, event: &str, fields: &[(&str, String)]) {
    let mut line = format!("[oubli] level={level} target={target} event={event}");
    for (key, value) in fields {
        line.push(' ');
        line.push_str(key);
        line.push('=');
        line.push_str(value);
    }
    eprintln!("{line}");
}

#[macro_export]
macro_rules! debug_event {
    ($target:expr, $event:expr $(,)?) => {{
        #[cfg(debug_assertions)]
        {
            let fields: [(&str, String); 0] = [];
            $crate::telemetry::emit("debug", $target, $event, &fields);
        }
    }};
    ($target:expr, $event:expr, $($key:literal = $value:expr),+ $(,)?) => {{
        #[cfg(debug_assertions)]
        {
            let fields = vec![$(($key, ($value).to_string())),+];
            $crate::telemetry::emit("debug", $target, $event, &fields);
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = ($(&$value),+, &$target, &$event);
        }
    }};
}

#[macro_export]
macro_rules! warn_event {
    ($target:expr, $event:expr $(,)?) => {{
        #[cfg(debug_assertions)]
        {
            let fields: [(&str, String); 0] = [];
            $crate::telemetry::emit("warn", $target, $event, &fields);
        }
    }};
    ($target:expr, $event:expr, $($key:literal = $value:expr),+ $(,)?) => {{
        #[cfg(debug_assertions)]
        {
            let fields = vec![$(($key, ($value).to_string())),+];
            $crate::telemetry::emit("warn", $target, $event, &fields);
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = ($(&$value),+, &$target, &$event);
        }
    }};
}
