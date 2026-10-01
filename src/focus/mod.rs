pub mod niri;

pub async fn get_focus_window_id() -> Option<u64> {
    niri::get_focus_window_id().await
}

pub async fn get_active_window_ids() -> Option<std::collections::HashSet<u64>> {
    niri::get_active_window_ids().await
}
