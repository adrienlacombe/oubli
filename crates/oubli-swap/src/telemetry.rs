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
macro_rules! swap_debug_event {
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
